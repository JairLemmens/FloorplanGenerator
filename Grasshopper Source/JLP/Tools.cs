using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Net.Http;
using System.Reflection.PortableExecutable;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Forms;
using Grasshopper.Kernel;
using Newtonsoft.Json;
using Rhino;
using Rhino.Geometry;
using Rhino.Render.DataSources;
using Rhino.UI;


namespace JLP
{
	public class Tools
	{	
		/// <summary>
		/// Defines controldata structure as used in the json instruction
		/// </summary>
		public class ControlData
		{
			public List<List<double>> control_matrix { get; set; } = new List<List<double>>();
			public List<List<List<double>>> geometries { get; set; } = new List<List<List<double>>>();
			public List<double?> aspect_ratios { get; set; } = new List<double?>();
			public List<List<double>> colours { get; set; } = new List<List<double>>();
			public List<bool> lock_trans {  get; set; } = new List<bool>();
		}

		public class OutputData
		{
			public byte[,] sample {get; }
			public List<double> offset {get; }
			public double scale { get; }
			public List<System.Drawing.Color> colours {get;}
			public List<JLP.DefineSpace> spaces { get; }
			public List<JLP.DefineConnection> connections {get;}
			public OutputData(byte[,] sample, List<double> offset, double scale, List<System.Drawing.Color> colours, List<DefineSpace> spaces, List<DefineConnection> connections)
			{
				this.sample = sample;
				this.offset = offset;
				this.scale = scale;
				this.colours = colours;
				this.spaces = spaces;
				this.connections = connections;
			}
		}
		public class SampleData
		{
			public int num_samples { get; }
			public int imsize { get; }
			public int num_spaces { get; }
			public byte[] shapes { get; }
			public byte[] transforms { get; }
			public byte[] offsets { get; }
			public SampleData(byte[] data, int num_samples, int num_spaces, int imsize)
			{
				this.num_samples = num_samples;
				this.imsize = imsize;
				this.num_spaces = num_spaces-1;
				int byteoffset = 0;

				shapes = new byte[num_samples * imsize * imsize];
				Array.Copy(data, shapes, shapes.Length);
				byteoffset += shapes.Length;

				transforms = new byte[num_samples * this.num_spaces * 3 * 4];
				Array.Copy(data, byteoffset, transforms,0,transforms.Length);
				byteoffset += transforms.Length;

				offsets = new byte[num_samples * 2 * 4];
				Array.Copy(data,byteoffset, offsets , 0 , offsets.Length);
				byteoffset += offsets.Length;
			}

		}

		/// <summary>
		/// Generates a list of coordinates from a 2D brep boundary
		/// </summary>
		/// <param name="brep"></param>
		/// <returns></returns>
		public static List<List<double>> GetBrepBoundaryCoords(Brep brep)
		{
			// This will store the boundary coordinates
			List<List<double>> boundaryCoords = new List<List<double>>();

			if (brep == null) return null;

			// Assume first face (or adapt if multiple faces)
			if (brep.Faces.Count == 0) return null;

			var face = brep.Faces[0];

			// Only handle planar faces
			if (!face.IsPlanar()) return null;

			// Loop through outer loops of the face
			foreach (var loop in face.Loops)
			{
				if (loop.LoopType != BrepLoopType.Outer) continue;

				Curve loopCurve = loop.To3dCurve();
				if (loopCurve == null) continue;

				// Convert curve to polyline approximation
				Polyline polyline;
				if (loopCurve.TryGetPolyline(out polyline))
				{
					foreach (var pt in polyline.Take(polyline.Count - 1))
					{
						boundaryCoords.Add(new List<double> { pt.X, pt.Y });
					}
				}
				else
				{
					// If curve is not a polyline, sample points along the curve
					int sampleCount = 50; // adjust as needed
					for (int i = 0; i <= sampleCount; i++)
					{
						double t = loopCurve.Domain.T0 + i * (loopCurve.Domain.T1 - loopCurve.Domain.T0) / sampleCount;
						Point3d pt = loopCurve.PointAt(t);
						boundaryCoords.Add(new List<double> { pt.X, pt.Y });
					}
				}
			}
			return boundaryCoords.Count > 0 ? boundaryCoords : null;
		}

		/// <summary>
		/// Generates an instruction json to be used inside of the floorplanquery
		/// </summary>
		/// <param name="space_id"></param>
		/// <param name="doc"></param>
		/// <returns></returns>
		public static (string, List<JLP.DefineSpace>,List<JLP.DefineConnection>) Create_json_instruction(GH_Component component, JLP.DefineSpace space_id, GH_Document doc)
		{	
			var spaces = doc.Objects.OfType<JLP.DefineSpace>().OrderBy(s => s.NickName).ToList();
			var connections = doc.Objects.OfType<JLP.DefineConnection>();
			ControlData controlData = new ControlData();
			List<List<List<double>>> geometries = new List<List<List<double>>>();
			List<double?> aspectRatios = new List<double?>();
			List<List<double>> colours = new List<List<double>>();
			List<bool> lock_trans = new List<bool>();

			List<JLP.DefineSpace> valid_spaces = new List<JLP.DefineSpace>();
			List<JLP.DefineConnection> valid_connections = new List<JLP.DefineConnection>();
			List<string> extra_spaces = new List<string>();
			double total_area = 0;
			double mask_area = 0;

			foreach (var space in spaces)
			{
				if (space == space_id)
				{
					if (space.shape != null)
					{
						geometries.Add(GetBrepBoundaryCoords(space.shape));
						space.area = space.shape.GetArea();
						mask_area = space.area;
						continue;
					}
					else
					{
						geometries.Add(null);
						continue;
					}
				}
				else if (space.parent_id == space_id)
				{
					space.ExpireSolution(true);
					valid_spaces.Add(space);
				}
			}

			foreach (var connection in connections)
			{
				if (valid_spaces.Contains(connection.spaceA))
				{	
					valid_connections.Add(connection);
					if (!valid_spaces.Contains(connection.spaceB) & (connection.spaceB!=null)) 
					{	
						if (connection.spaceB.shape == null)
						{
							component.AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, $"{connection.spaceB.NickName} does not have a geometry assigned");
							return (null, null, null);
						}
						valid_spaces.Add(connection.spaceB);
					}
				}
				else if (valid_spaces.Contains(connection.spaceB))
				{
					valid_connections.Add(connection);
					if (connection.spaceA.shape == null)
					{
						component.AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, $"{connection.spaceA.NickName} does not have a geometry assigned");
						return (null, null, null);
					}
					valid_spaces.Add(connection.spaceA);
				}
			}
			
			foreach (var space in valid_spaces)
			{
				// Geometry
				if (space.shape != null && space.shape.Faces.Count > 0)
				{
					geometries.Add(GetBrepBoundaryCoords(space.shape));
					space.area = space.shape.GetArea();
					total_area += space.area;
				}
				else
				{
					total_area += space.area;
					geometries.Add(null);
				}
				// Aspect ratio
				aspectRatios.Add(space.aspect);
				lock_trans.Add(space.lock_trans);
				// Colour
				colours.Add(new List<double> { space.colour.R, space.colour.G, space.colour.B });
			}

			//extract variables
			int n = valid_spaces.Count();
			List<List<double>> controlMatrix = new List<List<double>>();
			for (int i = 0; i < n; i++)
			{
				controlMatrix.Add(new List<double>(new double[n])); // initialize with zeros
			}

			for (int i = 0; i < n; i++)
			{	
				var spaceA = valid_spaces[i];
				Debug.WriteLine(spaceA.NickName);
				// Diagonal = area
				if (mask_area > 0)
				{
					Debug.WriteLine((mask_area / total_area));
					controlMatrix[i][i] = spaceA.area * (mask_area / total_area);
				}
				else
				{
					controlMatrix[i][i] = spaceA.area;
				}
				
				for (int j = i + 1; j < n; j++)
				{
					var spaceB = valid_spaces[j];
					foreach (var connection in valid_connections)
					{
						if ((connection.spaceA == spaceA) & (connection.spaceB == spaceB))
						{
							controlMatrix[i][j] = 1;
							controlMatrix[j][i] = 1;
						}
						else if ((connection.spaceB == spaceA) & (connection.spaceA == spaceB))
						{
							controlMatrix[i][j] = 1;
							controlMatrix[j][i] = 1;
						}
					}
				}
			}
			controlData.control_matrix = controlMatrix;
			controlData.geometries = geometries;
			controlData.aspect_ratios = aspectRatios;
			controlData.colours = colours;
			controlData.lock_trans = lock_trans;
			string json = JsonConvert.SerializeObject(controlData, Formatting.Indented);
			return (json,valid_spaces,valid_connections);
		}
	}
}
