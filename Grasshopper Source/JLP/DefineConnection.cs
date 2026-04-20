using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.Drawing;
using System.Linq;
using System.Windows.Forms;
using Grasshopper.GUI;
using Grasshopper.GUI.Canvas;
using Grasshopper.GUI.StringDisplay;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Attributes;
using Grasshopper.Kernel.Parameters;
using Grasshopper.Kernel.Types;
using Rhino.Geometry;
using Rhino.Render.DataSources;

namespace JLP
{
	public class DefineConnection : GH_Component
    {
		/// <summary>
		/// Initializes a new instance of the MyComponent1 class.
		/// </summary>
		public JLP.DefineSpace spaceA = null;
		public JLP.DefineSpace spaceB = null;

		public List<Rhino.Geometry.Polyline> boundary = new List<Rhino.Geometry.Polyline> { };
		public DefineConnection(): base("DefineConnection", "DC","Defines the connection between two spaces","JLP", "Generation") {}

        /// <summary>
        /// Registers all the input parameters for this component.
        /// </summary>
        protected override void RegisterInputParams(GH_Component.GH_InputParamManager pManager)
        {
			pManager.AddGenericParameter("Space A", "A", "Input space ID of space A", GH_ParamAccess.item);
			pManager.AddGenericParameter("Space B", "B", "Input space ID of space B", GH_ParamAccess.item);
			pManager[1].Optional = true;
		}

		/// <summary>
		/// Registers all the output parameters for this component.
		/// </summary>
		protected override void RegisterOutputParams(GH_Component.GH_OutputParamManager pManager)
        {
			pManager.AddCurveParameter("Boundary curves", "Bound", "Boundary curves between spaces A and B", GH_ParamAccess.list);
		}

		/// <summary>
		/// This is the method that actually does the work.
		/// </summary>
		/// <param name="DA">The DA object is used to retrieve from inputs and store in outputs.</param>
		protected override void SolveInstance(IGH_DataAccess DA)
        {
			spaceA = null;
			spaceB = null;
			DA.GetData(0, ref spaceA);
			DA.GetData(1, ref spaceB);
			DA.SetDataList(0, boundary);
		}

		/// <summary>
		/// Provides an Icon for the component.
		/// </summary>
		protected override System.Drawing.Bitmap Icon
        {
            get
            {
				//You can add image files to your project resources and access them like this:
				// return Resources.IconForThisComponent;
				return Properties.Resources.DefineConnection;
				//return null;
            }
        }

		public override void CreateAttributes()
		{
			m_attributes = new DefineConnectionAttributes(this);
		}

		/// <summary>
		/// Gets the unique ID for this component. Do not change this ID after release.
		/// </summary>
		public override Guid ComponentGuid
        {
            get { return new Guid("0FED3A4D-755E-43B7-BF2F-83D6F3D63E73"); }
        }
    }

	public class DefineConnectionAttributes : GH_ComponentAttributes
	{
		public DefineConnectionAttributes(GH_Component component) : base(component) {}

	}
}