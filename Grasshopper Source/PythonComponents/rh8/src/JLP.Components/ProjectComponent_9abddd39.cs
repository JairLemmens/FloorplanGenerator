using System;
using SD = System.Drawing;
using SWF = System.Windows.Forms;

using Rhino.Geometry;

using Grasshopper.Kernel;

namespace RhinoCodePlatform.Rhino3D.Projects.Plugin.GH
{
  public sealed class ProjectComponent_9abddd39 : ProjectComponent_Base
  {
    static readonly string s_scriptDataId = "9abddd39-c916-44fc-92a7-99f726ddf81e";
    static readonly string s_scriptIconData = "iVBORw0KGgoAAAANSUhEUgAAABgAAAAYCAYAAADgdz34AAAABGdBTUEAALGPC/xhBQAAAAlwSFlzAAAOwgAADsIBFShKgAAAAaZJREFUSEvt1E9LVFEYgPEnmXtFrRBJDFezkKhFmRZF2R+CaREkSISRBLkJjJIwbXTGqGWfoT5J36ll3ntnNvPEGStmzh2s0J39YODynBfO5rwDx4ZtLpqzEPcjYca0BWtmLNnijXtcjmcOxYw19zjb/Zbx3xd9TWbj2X9mxjVbPCj1cNGXE59MKxuSnovP/4pSMWc37oENFtyhJifPSLIiyaqMTMdzBzLjkd+5VOqfSaz3XyzDVUleSPpY7lZ6zwayoGrBatwDt1nsNJmPeyDpQ0mX4l5iwboyUeoNpjp1Xsb9F6nsypUk7n3MuWVBLe6BOzx3m2rcAxmuSeXgXVFGzanHPbDJVessxz2QidNSeRv3Ets8scX5uAe+Z9YGG26VF02SZUkvxL2PbebMWYl7Lz8y7juWfD206Y1krtv2X9DAB9HHNrOdFltmXI/PeimTfhvalOTp/m/4lYxNxXMDKafC5prTNOeOMlSayXlmwUz47jA6L2OlXfkjJbXgnjkfLLivjHR7wUy4IJ4/lE7OzfCyzFj8uR+T8cyRMOf2oL+O/465H5EextfDbBemAAAAAElFTkSuQmCC";

    public override Guid ComponentGuid { get; } = new Guid("9abddd39-c916-44fc-92a7-99f726ddf81e");

    public override GH_Exposure Exposure { get; } = GH_Exposure.primary;

    public override bool Obsolete { get; } = false;

    public ProjectComponent_9abddd39() : base(GetResource(s_scriptDataId), s_scriptIconData,
        name: "Data2Boundary",
        nickname: "Data2Boundary",
        description: @"",
        category: "JLP",
        subCategory: "Post Processing"
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
