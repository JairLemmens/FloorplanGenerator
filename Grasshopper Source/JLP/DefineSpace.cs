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

namespace JLP
{
	public class DefineSpace : GH_Component
    {
		/// <summary>
		/// Initializes a new instance of the MyComponent1 class.
		/// </summary>
		public JLP.DefineSpace parent_id = null;
		public Color colour = Color.FromArgb(255, 255, 255);
		public Rhino.Geometry.Brep shape = null;
		public double area = double.NaN;
		public double aspect = double.NaN;
		public bool lock_trans = true;
		public List<double> transform = null;
		public List<Rhino.Geometry.Polyline> boundary = new List<Rhino.Geometry.Polyline>{};

		public DefineSpace(): base("DefineSpace", "DS","Defines a space to be used by the generator","JLP", "Generation") {}

        /// <summary>
        /// Registers all the input parameters for this component.
        /// </summary>
        protected override void RegisterInputParams(GH_Component.GH_InputParamManager pManager)
        {
			pManager.AddGenericParameter("Parent ID", "PID", "Input space ID of parent", GH_ParamAccess.item);
			pManager[0].Optional = true;
			pManager.AddColourParameter("Colour", "clr", "Set colour to be used to show space", GH_ParamAccess.item,Color.FromArgb(255, 0, 0));

			pManager.AddBrepParameter("Shape", "Shp", "Specifies space footprint", GH_ParamAccess.item);
			pManager[2].Optional = true;

			pManager.AddNumberParameter("Area", "Area", "Specifies space area", GH_ParamAccess.item);
			pManager[3].Optional = true;
			
			pManager.AddNumberParameter("Aspect ratio", "AR", "Aspect ration of space", GH_ParamAccess.item);
			pManager[4].Optional = true;

			pManager.AddBooleanParameter("Lock transform", "LT", "This locks the transform of the reference shape", GH_ParamAccess.item, true);
			pManager[5].Optional = true;
		}

		/// <summary>
		/// Registers all the output parameters for this component.
		/// </summary>
		protected override void RegisterOutputParams(GH_Component.GH_OutputParamManager pManager)
        {
			pManager.AddGenericParameter("Space ID", "Space ID", "ID of space to be used in query or to be assigned as parent or adjacent", GH_ParamAccess.item);
			pManager.AddNumberParameter("Transform", "Transform", "contains x,y,rotation of space chosen in FloorplanQuery",GH_ParamAccess.list);
			pManager.AddCurveParameter("Exterior", "Exterior polylines", "contains the exterior polylines of the space", GH_ParamAccess.list);
		}

		/// <summary>
		/// This is the method that actually does the work.
		/// </summary>
		/// <param name="DA">The DA object is used to retrieve from inputs and store in outputs.</param>
		protected override void SolveInstance(IGH_DataAccess DA)
        {
			parent_id = null;
			colour = Color.FromArgb(255, 255, 255);
			shape = null;
			area = double.NaN;
			aspect = double.NaN;
			lock_trans = true;

			DA.GetData(0, ref parent_id);
			DA.GetData(1, ref colour);
			DA.GetData(2, ref shape);
			DA.GetData(3, ref area);
			DA.GetData(4, ref aspect);
			DA.GetData(5, ref lock_trans);

			if (double.IsNaN(area) & shape==null)
			{
				AddRuntimeMessage(GH_RuntimeMessageLevel.Warning,
					"Either area or shape must be assigned");
				return; // stops further computation
			}
			DA.SetData(0, this);
			DA.SetDataList(1, transform);
			DA.SetDataList(2, boundary);
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
				return Properties.Resources.Definespace;
				//return null;
            }
        }

		public override void CreateAttributes()
		{
			m_attributes = new DefineSpaceAttributes(this);
		}

		public override void AddedToDocument(GH_Document document)
		{	
			base.AddedToDocument(document);
			if (this.NickName == "DefineSpace")
			{
				this.NickName = "Name me"; // only changes after placement
				this.ExpireSolution(true);     // refresh the canvas
			}
		}
		/// <summary>
		/// Gets the unique ID for this component. Do not change this ID after release.
		/// </summary>
		public override Guid ComponentGuid
        {
            get { return new Guid("0FED3A4D-752E-43B7-BF2F-83D6F3D63E73"); }
        }
    }

	public class DefineSpaceAttributes : GH_ComponentAttributes
	{
		public DefineSpaceAttributes(GH_Component component) : base(component) {}

		protected override void Layout()
		{
			base.Layout();
			Owner.IconDisplayMode = GH_IconDisplayMode.name;
		}
	}
}