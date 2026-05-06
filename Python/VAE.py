import torch
import math
import torch.nn as nn
import numpy as np
import cv2 as cv

def canonicalize(geom,imsize = 64,target_size = 1024):
    translation = -geom.min(0)
    geom =geom +translation
    scale1 = (0.9*imsize)/geom.max()
    geom *=scale1
    geom+= 0.05*imsize

    geom = geom.astype(np.int32)
    img = np.zeros((imsize,imsize),dtype=float)
    cv.fillPoly(img,np.expand_dims(geom,0),1.0)
    scale2 = np.sqrt(target_size/img.sum())
    
    pixel_coords = np.stack(img.nonzero(),axis=1).astype(float)
    mean, eigenvectors  = cv.PCACompute(pixel_coords,mean=None)
    
    mean = np.roll(mean,1)
    angle = np.arctan2(eigenvectors[0,1], eigenvectors[0,0])
    dist = ((pixel_coords-mean)@[1,0])
    mass_positive = np.sum(dist > 0)
    mass_negative = np.sum(dist < 0)
    if mass_negative>mass_positive:
        angle+=np.pi

    angle+=0.5*np.pi
    R = np.array([[np.cos(angle), -np.sin(angle)],[np.sin(angle),  np.cos(angle)]])
    rot_scaled_geom = (R@(((geom-mean)*scale2).T)).T.astype(np.int32)+imsize//2
    rot_scaled_img = np.zeros((imsize,imsize),dtype=float)
    cv.fillPoly(rot_scaled_img,np.expand_dims(rot_scaled_geom,0),1.0)
    
    return(rot_scaled_img,rot_scaled_geom,angle,(mean[0]-imsize/2)/(scale1))

def world_to_canonical(grid,positions,scales,rotations):
    grid = grid-positions[:,None,:]
    
    spatial_scale = torch.sqrt(scales/0.72)[:,None,None]
    grid = grid/spatial_scale

    sin_t = torch.sin(rotations)[:,None]
    cos_t = torch.cos(rotations)[:,None]

    x = grid[...,0]
    y = grid[...,1]

    x_r = cos_t*x + sin_t*y
    y_r = -sin_t*x +cos_t*y
    
    canon = torch.stack([x_r,y_r],dim=-1) +0.5
    return(canon)

def patchify(x, p_h=8, p_w=8, s_h=8, s_w=8):
    b, c = x.shape[:2]
    x = x.unfold(2, p_h, s_h)
    x = x.unfold(3, p_w, s_w)
    x = x.permute(0,2, 3, 1, 4, 5)
    x = x.reshape(b, -1, c, p_h, p_w)
    return x

class SwiGLU(nn.Module):
    def __init__(self, dim):
        super().__init__()
        self.w1_2 = nn.Linear(dim,4*dim)
        self.w3 = nn.Linear(dim*2,dim)
    def forward(self,x):
        g,h = self.w1_2(x).chunk(2,-1)
        return(self.w3(nn.functional.silu(g)*h))

class FiLM(nn.Module):
    def __init__(self, cond_dim, dim,repeat_cond = False):
        super().__init__()
        self.FiLM = nn.Sequential(nn.Linear(cond_dim,2*dim),SwiGLU(2*dim))
        self.repeat_cond = repeat_cond
    def forward(self,x,cond):
        gamma, beta = self.FiLM(cond).chunk(2,-1)
        if self.repeat_cond:
            _,N,_ = x.shape
            gamma = gamma.unsqueeze(1).repeat(1,N,1)
            beta = beta.unsqueeze(1).repeat(1,N,1)
        return(x*(1+gamma*0.1)+ beta)
    
class Skipped_SwiGLU_FiLM(nn.Module):
    def __init__(self, dim, cond_dim = 1,repeat_cond=False):
        super().__init__()
        self.norm = nn.LayerNorm(dim)
        self.film = FiLM(cond_dim,dim,repeat_cond)
        self.swiglu = SwiGLU(dim)
        self.repeat_cond = repeat_cond
    def forward(self,x,cond=None):
        h = self.norm(x)
        if cond!= None:
            h = self.film(h,cond)
        return(x + self.swiglu(h))
    
class Self_Attention_Head(nn.Module):
    def __init__(self, dim, block_dim,masked = True,num_heads =4,cond_dim=1):
        super().__init__()
        self.norm = nn.LayerNorm(dim)
        self.film = FiLM(cond_dim,dim)
        self.qkv = nn.Linear(dim, dim*3, bias=False)
        self.attention = nn.MultiheadAttention(dim,num_heads=num_heads,batch_first=True)
        self.masked = masked
        self.register_buffer('tril', torch.tril(torch.ones(block_dim,block_dim)))
        self.project = nn.Linear(dim,dim)

    def forward(self,x,cond=None):
        h = self.norm(x)
        if cond != None:
            h = self.film(h,cond)
        q,k,v = self.qkv(h).chunk(3,-1)
        if self.masked:
            attn,attn_weights = self.attention(q,k,v,attn_mask=self.tril[:x.shape[-2],:x.shape[-2]])
        else:
            attn,attn_weights = self.attention(q,k,v)
        out = self.project(attn)
        return(x+out) 
 
class Transformer_swiglu(nn.Module):
    def __init__(self,input_dim,hidden_dim,num_stages=1,cond_dim=1,masked=True,mask_size = 200):
        super().__init__()
        self.proj = nn.Linear(input_dim,hidden_dim)
        stages = []
        for _ in range(num_stages):
            stages.append(nn.ModuleList([Self_Attention_Head(hidden_dim,mask_size,masked,cond_dim=cond_dim),Skipped_SwiGLU_FiLM(hidden_dim,cond_dim)]))
        self.stages= nn.ModuleList(stages)

    def forward(self,x,cond=None):
        h = self.proj(x)
        for stage in self.stages:
            h = stage[0](h,cond)
            h = stage[1](h,cond)
        return(h)

class SinusoidalPositionEmbedding2D(nn.Module):
    def __init__(self, n_patches, embed_dim):
        super().__init__()
        self.embed_dim  = embed_dim//2
        # X-axis specific values
        x_positions = self.get_x_positions(n_patches).reshape(-1, 1)                     
        x_pos_embedding = self.generate_sinusoidal1D(x_positions)                   

        # Y-axis specific values
        y_positions = self.get_y_positions(n_patches).reshape(-1, 1)                    
        y_pos_embedding = self.generate_sinusoidal1D(y_positions)                   

        # Combine x-axis and y-axis positional encodings
        pos_embedding = torch.cat((x_pos_embedding, y_pos_embedding), -1)           
        self.register_buffer("pos_embedding", pos_embedding)                  

    def get_x_positions(self,n_patches, start_idx=0):
        n_patches_ = int(n_patches ** 0.5)                                    
        x_positions = torch.arange(start_idx, n_patches_ + start_idx)         
        x_positions = x_positions.unsqueeze(0)                                
        x_positions = torch.repeat_interleave(x_positions, n_patches_, 0)     
        x_positions = x_positions.reshape(-1)                                
        return x_positions

    def get_y_positions(self,n_patches, start_idx=0):
        n_patches_ = int(n_patches ** 0.5)                                    
        y_positions = torch.arange(start_idx, n_patches_+start_idx)           
        y_positions = torch.repeat_interleave(y_positions, n_patches_, 0)     
        return y_positions
    
    def generate_sinusoidal1D(self, sequence):
        # Denominator
        denominator = torch.pow(10000, torch.arange(0, self.embed_dim, 2) / self.embed_dim) 

        pos_embedding = torch.zeros(1, sequence.shape[0], self.embed_dim)
        denominator = sequence / denominator                                                  
        pos_embedding[:, :, ::2]  = torch.sin(denominator)                                    
        pos_embedding[:, :, 1::2] = torch.cos(denominator)                                   
        return pos_embedding                                                                  

class FourierFeatures2D(nn.Module):
    def __init__(self, num_bands=4, max_freq=32.0):
        super().__init__()
        # frequencies: [1, 2, 4, ..., max_freq]
        self.freq_bands = 2.0 ** torch.linspace(0.0,math.log2(max_freq),steps=num_bands)
    def forward(self, x):
        """
        x: (batch, N, 2) coordinates in [0, 1]
        returns: (batch, N, 2 * 2 * num_bands)
        """
        freq = self.freq_bands.to(x.device).view(1, 1, -1)
        x = x.unsqueeze(-1)
        x_freq = 2 * math.pi * x * freq
        out = torch.cat([torch.sin(x_freq), torch.cos(x_freq)], dim=-1)
        # flatten last two dims → (B, N, 2 * 2 * num_bands)
        return out.view(x.shape[0], x.shape[1], -1)
    
class Implicit_decoder(nn.Module):
    def __init__(self, dim,cond_dim,num_layers = 5):
        super().__init__()
        self.fourierfeatures = FourierFeatures2D(num_bands=dim//4)
        assert dim%4 == 0, "dim must be divisible by 4"
        self.layers = nn.ModuleList([Skipped_SwiGLU_FiLM(dim,cond_dim,repeat_cond=True).to('cuda') for _ in range(num_layers)])
        self.project_out = nn.Linear(dim,1)
    def forward(self,coords,cond):
        h = self.fourierfeatures(coords)
        for layer in self.layers:
            h = layer(h,cond)
        return(self.project_out(h))

class Space(nn.Module):
    def __init__(self,latent,position,angle,scale=1,device= 'cuda'):
        super().__init__()
        """
        Input:
        latents: torch.tensor(latent_dim)
        position: float(2)
        position: float(1)
        scale: float(1)
        Obtain these variables by passing geometry through canonicalize
        or randomly initialize latents: torch.randn(latent_dim)
        """
        self.latent = nn.Parameter(latent.view(latent.shape[-1]))
        self.position = nn.Parameter(torch.tensor(position,device=device,dtype=torch.float))
        self.scale = nn.Parameter(torch.tensor(scale,device=device,dtype=torch.float))
        self.angle = nn.Parameter(torch.tensor(angle,device=device,dtype=torch.float))
        self.device = device
    def forward(self):
        return(self.latent,torch.concat([self.position,self.scale,torch.sin(self.angle),torch.cos(self.angle)],dim=-1))

class Spaces(nn.Module):
    def __init__(self,spaces:list[Space]):
        super().__init__()
        self.spaces = nn.ModuleList(spaces)

    @property
    def positions(self) -> torch.Tensor:
        return torch.stack([space.position for space in self.spaces], dim=0)   # (N,2)
    @positions.setter
    def positions(self, value):
        assert value.shape == (len(self.spaces),2)
        for i, s in enumerate(self.spaces):
            with torch.no_grad():
                s.position.copy_(value[i])

    @property
    def scales(self) -> torch.Tensor:
        return torch.stack([space.scale for space in self.spaces], dim=0)      # (N,)
    @scales.setter
    def scales(self, value):
        assert value.shape == (len(self.spaces),)
        for i, s in enumerate(self.spaces):
            with torch.no_grad():
                s.scale.copy_(value[i])

    @property
    def angles(self) -> torch.Tensor:
        return torch.stack([space.angle for space in self.spaces], dim=0)      # (N,)
    @angles.setter
    def angles(self, value):
        assert value.shape == (len(self.spaces),)
        for i, s in enumerate(self.spaces):
            with torch.no_grad():
                s.angle.copy_(value[i])

    
    @property
    def latents(self) -> torch.Tensor:
        return torch.stack([space.latent for space in self.spaces], dim=0)
    @latents.setter
    def latents(self, value):
        assert value.shape == self.latents.shape
        for i, s in enumerate(self.spaces):
            with torch.no_grad():
                s.latent.copy_(value[i])

    @property
    def transforms(self) -> torch.Tensor:
        return torch.cat([
            self.positions,
            self.scales.unsqueeze(-1),
            torch.sin(self.angles).unsqueeze(-1),
            torch.cos(self.angles).unsqueeze(-1),
        ], dim=-1)   # (N,5)

    def forward(self):
        return self.latents, self.transforms
    
class VIT_VAE_impl(nn.Module):
    def __init__(self,bottleneck_dim = 64, enc_hidden_dim = 64,dec_hidden_dim =32 ,enc_depth = 5,dec_depth = 5,num_patches=64,patch_size=64,device='cuda'):
        super().__init__()
        self.device = device
        self.bottleneck_dim = bottleneck_dim
        self.embedding = SinusoidalPositionEmbedding2D(num_patches,enc_hidden_dim).to(device)
        self.project_enc = nn.Linear(patch_size,enc_hidden_dim).to(device)
        self.encoder = nn.Sequential(Transformer_swiglu(enc_hidden_dim,enc_hidden_dim,enc_depth,masked=False),nn.Linear(enc_hidden_dim,bottleneck_dim*2)).to(device)
        self.decoder = Implicit_decoder(dec_hidden_dim,bottleneck_dim,dec_depth).to(device)
    def encode(self,canon_img):
        """
        Input: canonicalized tensor
        Output: Space class instance
        """
        patches = patchify(torch.tensor(canon_img,device = self.device,dtype=torch.float).view(-1,1,64,64))
        B,P,N,Px,Py = patches.shape
        patches = patches.view(B,P,-1)
        h = self.project_enc(patches) + self.embedding.pos_embedding.detach()
        bottle = self.encoder(h).mean(dim=1)
        z = bottle[:, :self.bottleneck_dim]
        return(z)
    def generate_space(self,spaces:Spaces,imsize=64):
        """
        Input: Spaces [N,]
        Output: SDF of shape[N,imsize,imsize] 
        """
        B,C = spaces.latents.shape
        y = torch.linspace(0,1,imsize, device='cuda')
        x = torch.linspace(0,1,imsize, device=y.device)
        yy, xx = torch.meshgrid(y, x, indexing='ij')

        coords = torch.stack([xx, yy], dim=-1).flatten(0,1)
        coords = coords.unsqueeze(0).repeat(B,1,1)

        dx = xx[None] - spaces.positions[:,0][:,None,None]
        dy = yy[None] - spaces.positions[:,1][:,None,None]
        radial = 2*torch.sqrt(dx*dx + dy*dy + 1e-6)
        radius = 0.332 * torch.sqrt(spaces.scales[:, None, None])
        baseline = radial - radius

        residual = vit_vae.decoder(world_to_canonical(coords,spaces.positions,spaces.scales,spaces.angles),spaces.latents).view(B,imsize,imsize)
        recon = residual*torch.sqrt(spaces.scales/0.72)[:,None,None] + baseline.detach()
        return(recon)
