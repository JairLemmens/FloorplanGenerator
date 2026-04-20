using System;
using System.IO;
using System.Text;
using System.Reflection;
using System.Resources;
using SD = System.Drawing;

using Rhino;
using Grasshopper.Kernel;

namespace RhinoCodePlatform.Rhino3D.Projects.Plugin.GH
{
  public sealed class AssemblyInfo : GH_AssemblyInfo
  {
    static readonly string s_assemblyIconData = "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAABGdBTUEAALGPC/xhBQAAAAlwSFlzAAAOwgAADsIBFShKgAAAAaZJREFUSEvt1E9LVFEYgPEnmXtFrRBJDFezkKhFmRZF2R+CaREkSISRBLkJjJIwbXTGqGWfoT5J36ll3ntnNvPEGStmzh2s0J39YODynBfO5rwDx4ZtLpqzEPcjYca0BWtmLNnijXtcjmcOxYw19zjb/Zbx3xd9TWbj2X9mxjVbPCj1cNGXE59MKxuSnovP/4pSMWc37oENFtyhJifPSLIiyaqMTMdzBzLjkd+5VOqfSaz3XyzDVUleSPpY7lZ6zwayoGrBatwDt1nsNJmPeyDpQ0mX4l5iwboyUeoNpjp1Xsb9F6nsypUk7n3MuWVBLe6BOzx3m2rcAxmuSeXgXVFGzanHPbDJVessxz2QidNSeRv3Ets8scX5uAe+Z9YGG26VF02SZUkvxL2PbebMWYl7Lz8y7juWfD206Y1krtv2X9DAB9HHNrOdFltmXI/PeimTfhvalOTp/m/4lYxNxXMDKafC5prTNOeOMlSayXlmwUz47jA6L2OlXfkjJbXgnjkfLLivjHR7wUy4IJ4/lE7OzfCyzFj8uR+T8cyRMOf2oL+O/465H5EextfDbBemAAAAAElFTkSuQmCC";
    static readonly string s_categoryIconData = "iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAABGdBTUEAALGPC/xhBQAAAAlwSFlzAAAOwgAADsIBFShKgAAAAQpJREFUOE/dkUFLAlEYRU+GI1TQIjSiKCgoKohwUQRBEFSLoKgY923bRjq06f/0H/pRpo4zKvcLG5zeDBpu2nR299yP9x7vg7/CjBlF7Ob91KjLlWKqgxA/brGf739FLSrqcZbmiD29Fl5EaSs7OQEL8d3ce+bYGqyLYtXwfLFQdvsM6nCkkI1Rtndm+3XuMzN4D0Zp03VJIUqKuHFdP+DSGiy6bvgKN6eozbUZc6PcDFhSnYvMDN6OKB64LinarFjIaca9UbEAP3762cLk2zvcWpvDvB8Sf/CoQrEmvDsxv5zvU9Rke7gB9ThJXYc1hUke+3HjkFj9PqjL+aBFLd9PjURZn0ze+f/gCxwhahyEGjr0AAAAAElFTkSuQmCC";

    public static readonly SD.Bitmap PluginIcon = default;
    public static readonly SD.Bitmap PluginCategoryIcon = default;

    static AssemblyInfo()
    {
      if (!s_assemblyIconData.Contains("ASSEMBLY-ICON"))
      {
        using (var aicon = new MemoryStream(Convert.FromBase64String(s_assemblyIconData)))
          PluginIcon = new SD.Bitmap(aicon);
      }

      if (!s_categoryIconData.Contains("ASSEMBLY-CATEGORY-ICON"))
      {
        using (var cicon = new MemoryStream(Convert.FromBase64String(s_categoryIconData)))
          PluginCategoryIcon = new SD.Bitmap(cicon);
      }
    }

    public override Guid Id { get; } = new Guid("2ceebce6-c31e-4289-abb1-b9cd21b65791");

    public override string AssemblyName { get; } = "JLP.Components";
    public override string AssemblyVersion { get; } = "0.1.9607.32983";
    public override string AssemblyDescription { get; } = @"";
    public override string AuthorName { get; } = "Jair Lemmens";
    public override string AuthorContact { get; } = "JairLemmens@gmail.com";
    public override GH_LibraryLicense AssemblyLicense { get; } = GH_LibraryLicense.unset;
    public override SD.Bitmap AssemblyIcon { get; } = PluginIcon;
  }

  public class ProjectComponentPlugin : GH_AssemblyPriority
  {
    static readonly Guid s_projectId = new Guid("2ceebce6-c31e-4289-abb1-b9cd21b65791");
    static readonly dynamic s_projectServer = default;
    static readonly object s_project = default;

    static ProjectComponentPlugin()
    {
      s_projectServer = ProjectInterop.GetProjectServer();
      if (s_projectServer is null)
      {
        RhinoApp.WriteLine($"Error loading Grasshopper plugin. Missing Rhino3D platform");
        return;
      }

      // get project
      dynamic dctx = ProjectInterop.CreateInvokeContext();
      dctx.Inputs["projectAssembly"] = typeof(ProjectComponentPlugin).Assembly;
      dctx.Inputs["projectId"] = s_projectId;
      dctx.Inputs["projectData"] = GetProjectData();

      object project = default;
      if (s_projectServer.TryInvoke("plugins/v1/deserialize", dctx)
            && dctx.Outputs.TryGet("project", out project))
      {
        // server reports errors
        s_project = project;
      }
    }

    public override GH_LoadingInstruction PriorityLoad()
    {
      if (AssemblyInfo.PluginCategoryIcon is SD.Bitmap icon)
      {
        Grasshopper.Instances.ComponentServer.AddCategoryIcon("JLP", icon);
      }
      Grasshopper.Instances.ComponentServer.AddCategorySymbolName("JLP", "JLP"[0]);

      return GH_LoadingInstruction.Proceed;
    }

    public static bool TryCreateScript(GH_Component ghcomponent, string serialized, out object script)
    {
      script = default;

      if (s_projectServer is null) return false;

      dynamic dctx = ProjectInterop.CreateInvokeContext();
      dctx.Inputs["component"] = ghcomponent;
      dctx.Inputs["project"] = s_project;
      dctx.Inputs["scriptData"] = serialized;

      if (s_projectServer.TryInvoke("plugins/v1/gh/deserialize", dctx))
      {
        return dctx.Outputs.TryGet("script", out script);
      }

      return false;
    }

    public static void DisposeScript(GH_Component ghcomponent, object script)
    {
      if (script is null)
        return;

      dynamic dctx = ProjectInterop.CreateInvokeContext();
      dctx.Inputs["component"] = ghcomponent;
      dctx.Inputs["project"] = s_project;
      dctx.Inputs["script"] = script;

      if (!s_projectServer.TryInvoke("plugins/v1/gh/dispose", dctx))
        throw new Exception("Error disposing Grasshopper script component");
    }

    static string GetProjectData()
    {
      var rm = new ResourceManager("Plugin.Data", Assembly.GetExecutingAssembly());
      return rm.GetString("PROJECT-DATA");
    }
  }
}
