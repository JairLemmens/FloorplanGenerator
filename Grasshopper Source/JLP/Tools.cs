using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Net.Http;

using System.Text;
using System.Threading.Tasks;
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
		public static string Create_json_instruction(GH_Component component, string space_id, GH_Document doc)
		{
			var spaces = doc.Objects.OfType<JLP.DefineSpace>().OrderBy(s => s.NickName).ToList();

			ControlData controlData = new ControlData();
			List<List<List<double>>> geometries = new List<List<List<double>>>();
			List<double?> aspectRatios = new List<double?>();
			List<List<double>> colours = new List<List<double>>();
			List<bool> lock_trans = new List<bool>();

			List<JLP.DefineSpace> valid_spaces = new List<JLP.DefineSpace>();
			List<string> extra_spaces = new List<string>();
			
			int hierarchal_depth = space_id.Count(c => c == '|') + 1;
			foreach (var space in spaces)
			{
				if (space.id == null)
				{
					space.ExpireSolution(true);
				}
				if (space.id != space_id)
				{
					space.ExpireSolution(true);
				}
				else
				{
					if (space.shape != null)
					{
						geometries.Add(GetBrepBoundaryCoords(space.shape));
						space.area = space.shape.GetArea();
						continue;
					}
					else
					{
						geometries.Add(null);
						continue;
					}
				}
				
				int depth = space.id.Count(c => c == '|');
				if (space.id.Contains(space_id) & (depth == hierarchal_depth))
				{
					valid_spaces.Add(space);
					foreach (string adj_s in space.adjacent)
					{
						if (!adj_s.Contains(space_id) || (adj_s.Count(c => c == '|') != hierarchal_depth))
						{
							extra_spaces.Add(adj_s);
						}
					}
				}
			}
			foreach (var space in spaces)
			{
				if (extra_spaces.Contains(space.id))
				{
					if (space.shape == null)
					{
						component.AddRuntimeMessage(GH_RuntimeMessageLevel.Warning, $"{space.id} does not have a geometry assigned");
						return null;
					}
					valid_spaces.Add(space);
				}
			}

			foreach (var space in valid_spaces)
			{
				// Geometry
				if (space.shape != null && space.shape.Faces.Count > 0)
				{
					geometries.Add(GetBrepBoundaryCoords(space.shape));
					space.area = space.shape.GetArea();
				}
				else
				{
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
				// Diagonal = area
				controlMatrix[i][i] = spaceA.area;

				for (int j = i + 1; j < n; j++)
				{
					var spaceB = valid_spaces[j];
					// 1 if spaceA is next to spaceB, 0 otherwise
					double valueAtoB = spaceA.adjacent.Any(s =>
						s.Equals(spaceB.NickName, StringComparison.Ordinal) ||
						s.EndsWith("|" + spaceB.NickName, StringComparison.Ordinal)) ? 1.0 : 0.0;

					double valueBtoA = spaceB.adjacent.Any(s =>
						s.Equals(spaceA.NickName, StringComparison.Ordinal) ||
						s.EndsWith("|" + spaceA.NickName, StringComparison.Ordinal)) ? 1.0 : 0.0;
					// Take the maximum for symmetry
					double finalValue = Math.Max(valueAtoB, valueBtoA);

					controlMatrix[i][j] = finalValue;
					controlMatrix[j][i] = finalValue; // mirror across diagonal
				}
			}

			controlData.control_matrix = controlMatrix;
			controlData.geometries = geometries;
			controlData.aspect_ratios = aspectRatios;
			controlData.colours = colours;
			controlData.lock_trans = lock_trans;
			string json = JsonConvert.SerializeObject(controlData, Formatting.Indented);
			return json;
		}
	}
}
