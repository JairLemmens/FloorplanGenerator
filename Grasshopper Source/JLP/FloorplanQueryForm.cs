using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Drawing;
using System.IO;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Diagnostics;
using System.Windows.Forms;
using System.Net.Http;
using Newtonsoft.Json.Linq;
using static JLP.Tools;

namespace JLP
{
	public partial class FloorplanQueryForm : Form
	{	
		public int SelectedIndex { get; private set; } = -1;
		public byte[,] SelectedSample = null;
		public float[,] Transforms = null;
		public float[] offset = null;
		private PictureBox _selectedPictureBox = null;
		public string instruction_json = null;
		public string api_key = null;
		public SampleData sample_data = null;
		public HttpClient http_client = new HttpClient();

		public FloorplanQueryForm(string recieved_instruction_json, string recieved_api_key, SampleData received_sample_data)
		{	
			InitializeComponent();
			http_client.Timeout = TimeSpan.FromSeconds(5);
			instruction_json  = recieved_instruction_json;
			api_key = recieved_api_key;
			sample_data = received_sample_data;
		}

		private void FloorplanQueryForm_Load(object sender, EventArgs e)
		{
			if (sample_data != null){
				LoadSamples();
			}
		}

		private Color[] BuildLookup(int[,] clrs)
		{	
			Color[] lookup = new Color[256];
			for (int i = 0; i < clrs.Length/3; i++)
			{
				lookup[i] = Color.FromArgb(clrs[i,0], clrs[i, 1], clrs[i, 2]);
			}
			return lookup;
		}

		private Bitmap CreateBitmapFromArray(byte[,] imgData, Color[] lookup)
		{
			int width = imgData.GetLength(1);
			int height = imgData.GetLength(0);

			Bitmap bmp = new Bitmap(width, height);

			for (int y = 0; y < height; y++)
			{
				for (int x = 0; x < width; x++)
				{
					byte index = imgData[y, x];
					bmp.SetPixel(x, y, lookup[index]);
				}
			}

			return bmp;
		}

		public void LoadSamples()
		{
			VariantsPanel.Controls.Clear();

			//import samples from binary
			
			int idx = 0;
			for (int i = 0; i < sample_data.num_samples; i++)
			{
				//add new image to the view for each i in num_samples
				byte[,] sample = new byte[sample_data.imsize, sample_data.imsize];
				for (int y = 0; y < sample_data.imsize; y++)
					for (int x = 0; x < sample_data.imsize; x++)
						sample[y, x] = sample_data.shapes[idx++];

				Color[] lookup = BuildLookup(JObject.Parse(instruction_json)["colours"].ToObject<int[,]>());
				var bmp = CreateBitmapFromArray(sample, lookup);

				var pic = new PictureBox
				{
					Width = sample_data.imsize+32,
					Height = sample_data.imsize + 32,
					SizeMode = PictureBoxSizeMode.CenterImage,
					Image = bmp,
					Tag = i,
					Cursor = Cursors.Hand,
					Margin = new Padding(5)
				};

				pic.Click += (s, e) =>
				{
					SelectedIndex = (int)((PictureBox)s).Tag;
					if (_selectedPictureBox != null)
					{
						_selectedPictureBox.SizeMode = (PictureBoxSizeMode.CenterImage);
					}
					_selectedPictureBox = (PictureBox)s;
					_selectedPictureBox.SizeMode = (PictureBoxSizeMode.Zoom);
				};

				VariantsPanel.Controls.Add(pic);
			}

		}

        private void Confirm_Click(object sender, EventArgs e)
        {
			if ((sample_data != null) & (SelectedIndex != -1))
			{	
				SelectedSample = new byte[sample_data.imsize, sample_data.imsize];
				int idx = SelectedIndex * sample_data.imsize * sample_data.imsize;
				for (int y = 0; y < sample_data.imsize; y++)
					for (int x = 0; x < sample_data.imsize; x++)
						SelectedSample[y, x] = sample_data.shapes[idx++];
				
				Transforms = new float[sample_data.num_spaces, 3];
				float[] temp = new float[sample_data.num_spaces * 3];
				Buffer.BlockCopy(sample_data.transforms, SelectedIndex * sample_data.num_spaces * 3 * 4, temp, 0, temp.Length * 4);
				int t = 0;
				for (int space_idx = 0; space_idx < sample_data.num_spaces; space_idx++)
				{
					for (int x = 0; x < 3; x++)
					{
						Transforms[space_idx, x] = temp[t++];
					}
				}

				offset = new float[2];
				Buffer.BlockCopy(sample_data.offsets, SelectedIndex * 2 * 4 , offset, 0, 2 * 4);

				this.DialogResult = DialogResult.OK;
				http_client.Dispose();
				this.Close();
			}
		}

        private void Cancel_Click(object sender, EventArgs e)
        {
			this.DialogResult = DialogResult.Cancel;
			http_client.Dispose();
			this.Close();
		}

		private async void SendRequestButton_Click(object sender, EventArgs e)
		{
			SendRequestButton.Enabled = false;
			try
			{	
				this.Status.Text = "Request sent";
				sample_data = await PostJsonAsync("https://jair.app/inference",instruction_json,api_key);
				this.Status.Text = "Recieved response";
				LoadSamples();
			}
			catch (Exception ex)
			{
				this.Status.Text = $"Error: {ex.Message}";
				Debug.WriteLine($"Error: {ex.Message}");
			}
			finally
			{
				SendRequestButton.Enabled = true;
			}
		}
		
		public async Task<SampleData> PostJsonAsync(string url, string json, string api_key)
		{
			var request = new HttpRequestMessage(HttpMethod.Post, url);
			request.Headers.Add("api-key", api_key);
			request.Content = new StringContent(json, Encoding.UTF8, "application/json");
			try
			{
				HttpResponseMessage response = await http_client.SendAsync(request,HttpCompletionOption.ResponseHeadersRead);
				response.EnsureSuccessStatusCode();
				var stream = await response.Content.ReadAsStreamAsync();
				var ms = new MemoryStream();
				await stream.CopyToAsync(ms);
				return new SampleData(ms.ToArray(), int.Parse(response.Headers.GetValues("num_samples").First()), int.Parse(response.Headers.GetValues("num_spaces").First()), int.Parse(response.Headers.GetValues("imsize").First()));
			}
			catch (TaskCanceledException)
			{
				throw new TimeoutException("No response from server within timeout period");
			}
			catch (HttpRequestException ex)
			{
				throw new InvalidOperationException("Failed", ex);
			}
		}
	}
}

