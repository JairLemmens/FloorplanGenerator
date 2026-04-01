using System;
using System.Collections.Generic;
using System.Drawing.Text;
using System.Linq;
using System.Net.Http;
using System.Windows.Forms;
using Grasshopper.GUI;
using Grasshopper.GUI.Canvas;
using Grasshopper.Kernel;
using Grasshopper.Kernel.Attributes;
using Grasshopper.Kernel.Types;
using Newtonsoft.Json.Linq;
using static JLP.Tools;

namespace JLP
{	
    public class FloorplanQuery : GH_Component
    {
        /// <summary>
        /// Initializes a new instance of the MyComponent1 class.
        /// </summary>
        
        public byte [,] sample= null;
		public string space_id = null;
        public string instruction_json = null;
		public string api_key = null;
		public SampleData sample_data = null;
		public List<System.Drawing.Color> colours = new List<System.Drawing.Color>();
		public FloorplanQuery()
          : base("FloorplanQuery", "FloorplanQuery",
              "Sends a request to the floorplan generation server.",
              "JLP", "Generation")
        {
        }

        /// <summary>
        /// Registers all the input parameters for this component.
        /// </summary>
        protected override void RegisterInputParams(GH_Component.GH_InputParamManager pManager)
        {
            pManager.AddTextParameter("Space ID", "ID","Requires Space ID of the to be generated space", GH_ParamAccess.item);
			pManager[0].Optional = true;
			pManager.AddTextParameter("Instruction JSON", "json", "Input instruction json", GH_ParamAccess.item);
			pManager[1].Optional = true;
			pManager.AddTextParameter("API Key", "key", "Input API key", GH_ParamAccess.item);
		}

		/// <summary>
		/// Registers all the output parameters for this component.
		/// </summary>
		protected override void RegisterOutputParams(GH_Component.GH_OutputParamManager pManager)
        {
			pManager.AddGenericParameter("data", "data", "byte_array", GH_ParamAccess.tree);
			pManager.AddTextParameter("Instruction JSON", "json", "JSON containing instruction for model", GH_ParamAccess.tree);
			pManager.AddColourParameter("Colours", "Colours", "Contains a list of colours for the spaces", GH_ParamAccess.list);
		}

		/// <summary>
		/// This is the method that actually does the work.
		/// </summary>
		/// <param name="DA">The DA object is used to retrieve from inputs and store in outputs.</param>
		protected override void SolveInstance(IGH_DataAccess DA)
        {
			space_id = null;
			instruction_json = null;
			api_key = null;
			colours = new List<System.Drawing.Color>();

			DA.GetData(0, ref space_id);
			DA.GetData(1, ref instruction_json);
			DA.GetData(2, ref api_key);

			if (space_id == null & instruction_json == null)
			{
				AddRuntimeMessage(GH_RuntimeMessageLevel.Warning,
					"Either Space ID or Instruction JSON has to be set");
				return; // stops further computation
			}

			if (space_id != null)
			{
				GH_Document doc = OnPingDocument();
				instruction_json = Create_json_instruction(this, space_id, doc);
			}
		
			var clrs = JObject.Parse(instruction_json)["colours"].ToObject<int[,]>();
			for (int i = 0; i < clrs.Length / 3; i++)
			{
				colours.Add(System.Drawing.Color.FromArgb(clrs[i, 0], clrs[i, 1], clrs[i, 2]));
			}

			DA.SetData(0, sample);
			DA.SetData(1, instruction_json);
			DA.SetDataList(2, colours);
		}

        /// <summary>
        /// Provides an Icon for the component.
        /// </summary>
        protected override System.Drawing.Bitmap Icon
        {
            get
            {
                //You can add image files to your project resources and access them like this:
                return Properties.Resources.Floorplanquery;
                //return null;
            }
        }

		public override void CreateAttributes()
		{
			m_attributes = new FloorplanQueryAttributes(this);
		}

		/// <summary>
		/// Gets the unique ID for this component. Do not change this ID after release.
		/// </summary>
		public override Guid ComponentGuid
        {
            get { return new Guid("0FED3A4D-752E-43B7-BF2F-83D6F3A63E73"); }
        }
    }

	public class FloorplanQueryAttributes : GH_ComponentAttributes
	{
		
		public FloorplanQueryAttributes(GH_Component owner) : base(owner) { }

		public override GH_ObjectResponse RespondToMouseDoubleClick(GH_Canvas sender, GH_CanvasMouseEvent e)
		{	
			var comp = (FloorplanQuery)Owner;
			comp.ExpireSolution(true);
			var form = new FloorplanQueryForm(comp.instruction_json,comp.api_key,comp.sample_data);
			if (form.ShowDialog() == DialogResult.OK)
			{  
				comp.sample = form.SelectedSample;
				comp.sample_data = form.sample_data;
				Owner.ExpireSolution(true);
			}

			return GH_ObjectResponse.Handled;
		}
	}
}