using System;
using SD = System.Drawing;
using SWF = System.Windows.Forms;

using Rhino.Geometry;

using Grasshopper.Kernel;

namespace RhinoCodePlatform.Rhino3D.Projects.Plugin.GH
{
  public sealed class ProjectComponent_5412231e : ProjectComponent_Base
  {
    static readonly string s_scriptDataId = "5412231e-7db7-4490-bc5b-e93ad7b7fcc3";
    static readonly string s_scriptIconData = "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAABGdBTUEAALGPC/xhBQAAAAlwSFlzAAAOwgAADsIBFShKgAAAATBJREFUSEvtlK1OxEAUhc9b8BaYfQAwvAVYngCHAI9CIOoIYEiwIDCgUDVrwKEQOGa2YOeSM1DSOduf2VIHX5p095vJPem9nQJ/iuBwZx+YqZ8E8yhZPN4X2NX1X2EeRbNo/O9RpLtGwsJtxb59qX4l6paor+F6nMsCG7qWxcsF3tS1YQ735nCgvhcDSgNmts+fw5jHIZ9GfSsGFIbGUHNDKmwOhrAwA5Z8Rog5zK3CuvofYkvQPdSnk+6ZmMNZ8NhRnxDQ/XjPx3i/vMG2ehI89szhSH0rFq+Uh1M8Xl/hXD0xh63gcau+l2YICzMg3fGFVVgLDq/qs2AIW8LW6FoNizNEfTZ9B41tYXvUr4z55ZlwoBys+tE0Q/gq8pVMd0wAQ3iIeJh0bTIGPwP/jOETB3as8eK788cAAAAASUVORK5CYII=";

    public override Guid ComponentGuid { get; } = new Guid("5412231e-7db7-4490-bc5b-e93ad7b7fcc3");

    public override GH_Exposure Exposure { get; } = GH_Exposure.primary;

    public override bool Obsolete { get; } = false;

    public ProjectComponent_5412231e() : base(GetResource(s_scriptDataId), s_scriptIconData,
        name: "Data2Boundary",
        nickname: "Data2Boundary",
        description: @"",
        category: "JLP",
        subCategory: "PostProcessing"
        )
    {
    }

    protected override void AppendAdditionalComponentMenuItems(SWF.ToolStripDropDown menu)
    {
      base.AppendAdditionalComponentMenuItems(menu);
      if (m_script is null) return;
      m_script.AppendAdditionalMenuItems(this, menu);
    }

    protected override void RegisterInputParams(GH_InputParamManager _) { }

    protected override void RegisterOutputParams(GH_OutputParamManager _) { }

    protected override void BeforeSolveInstance()
    {
      if (m_script is null) return;
      m_script.BeforeSolve(this);
    }

    protected override void SolveInstance(IGH_DataAccess DA)
    {
      if (m_script is null) return;
      m_script.Solve(this, DA);
    }

    protected override void AfterSolveInstance()
    {
      if (m_script is null) return;
      m_script.AfterSolve(this);
    }

    public override void RemovedFromDocument(GH_Document document)
    {
      ProjectComponentPlugin.DisposeScript(this, m_script);
      base.RemovedFromDocument(document);
    }

    public override BoundingBox ClippingBox
    {
      get
      {
        if (m_script is null) return BoundingBox.Empty;
        return m_script.GetClipBox(this);
      }
    }

    public override void DrawViewportWires(IGH_PreviewArgs args)
    {
      if (m_script is null) return;
      m_script.DrawWires(this, args);
    }

    public override void DrawViewportMeshes(IGH_PreviewArgs args)
    {
      if (m_script is null) return;
      m_script.DrawMeshes(this, args);
    }
  }
}
