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

namespace UIClassLibrary
{
	public partial class FloorplanQueryForm : Form
	{	
		public int SelectedIndex { get; private set; } = -1;
		public byte[,] SelectedSample = new byte[64, 64];
		private PictureBox _selectedPictureBox = null;
		//private byte[] raw = File.ReadAllBytes(@"C:\Users\jairl\Documents\EigenProjecten\SurrogateModel\img_array.bin");
		public byte[] sample_data = null;
		public string instruction_json = null;
		public string api_key = null;
		public HttpClient http_client = new HttpClient();
		public FloorplanQueryForm(string recieved_instruction_json, string recieved_api_key, byte[] received_sample_data)
		{	
			InitializeComponent();
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
			
			int num_samples = sample_data.Length / (64 * 64);

			int idx = 0;
			for (int i = 0; i < num_samples; i++)
			{
				//add new image to the view for each i in num_samples
				byte[,] sample = new byte[64, 64];
				for (int y = 0; y < 64; y++)
					for (int x = 0; x < 64; x++)
						sample[y, x] = sample_data[idx++];

				Color[] lookup = BuildLookup(JObject.Parse(instruction_json)["colours"].ToObject<int[,]>());
				var bmp = CreateBitmapFromArray(sample, lookup);

				var pic = new PictureBox
				{
					Width = 96,
					Height = 96,
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
				int idx = SelectedIndex * 64 * 64;
				for (int y = 0; y < 64; y++)
					for (int x = 0; x < 64; x++)
						SelectedSample[y, x] = sample_data[idx++];
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
			try
			{
				this.Status.Text = "Request sent";
				sample_data = await PostJsonAsync("https://jair.app/inference",instruction_json,api_key);
				this.Status.Text = "Recieved response";
				Debug.WriteLine($"Recieved {sample_data.Length} bytes from server");
				LoadSamples();
			}
			catch (Exception ex)
			{
				this.Status.Text = $"Error: {ex.Message}";
				Debug.WriteLine($"Error: {ex.Message}");
			}
		}

		public async Task<byte[]> PostJsonAsync(string url, string json, string api_key)
		{
			http_client.Timeout = TimeSpan.FromSeconds(10);
			http_client.DefaultRequestHeaders.Add("api-key", api_key);
			var content = new StringContent(json, Encoding.UTF8, "application/json");
			try
			{
				HttpResponseMessage response = await http_client.PostAsync(url, content);
				response.EnsureSuccessStatusCode();
				return await response.Content.ReadAsByteArrayAsync();
			}
			catch (TaskCanceledException)
			{
				// usually indicates a timeout
				throw new TimeoutException("No response from server within timeout period");
			}
			catch (HttpRequestException ex)
			{
				// network/connection issues
				throw new InvalidOperationException("Failed ", ex);
			}
		}
	}
}

