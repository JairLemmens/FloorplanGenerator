namespace UIClassLibrary
{
    partial class FloorplanQueryForm
	{
        /// <summary>
        /// Required designer variable.
        /// </summary>
        private System.ComponentModel.IContainer components = null;

        /// <summary>
        /// Clean up any resources being used.
        /// </summary>
        /// <param name="disposing">true if managed resources should be disposed; otherwise, false.</param>
        protected override void Dispose(bool disposing)
        {
            if (disposing && (components != null))
            {
                components.Dispose();
            }
            base.Dispose(disposing);
        }

        #region Windows Form Designer generated code

        /// <summary>
        /// Required method for Designer support - do not modify
        /// the contents of this method with the code editor.
        /// </summary>
        private void InitializeComponent()
        {
			System.ComponentModel.ComponentResourceManager resources = new System.ComponentModel.ComponentResourceManager(typeof(FloorplanQueryForm));
			this.VariantsPanel = new System.Windows.Forms.FlowLayoutPanel();
			this.toolStrip1 = new System.Windows.Forms.ToolStrip();
			this.SendRequestButton = new System.Windows.Forms.ToolStripButton();
			this.Confirm = new System.Windows.Forms.ToolStripButton();
			this.Cancel = new System.Windows.Forms.ToolStripButton();
			this.Status = new System.Windows.Forms.ToolStripTextBox();
			this.toolStrip1.SuspendLayout();
			this.SuspendLayout();
			// 
			// VariantsPanel
			// 
			this.VariantsPanel.AutoScroll = true;
			this.VariantsPanel.AutoSize = true;
			this.VariantsPanel.AutoSizeMode = System.Windows.Forms.AutoSizeMode.GrowAndShrink;
			this.VariantsPanel.Dock = System.Windows.Forms.DockStyle.Fill;
			this.VariantsPanel.Location = new System.Drawing.Point(0, 0);
			this.VariantsPanel.MinimumSize = new System.Drawing.Size(256, 256);
			this.VariantsPanel.Name = "VariantsPanel";
			this.VariantsPanel.Size = new System.Drawing.Size(800, 450);
			this.VariantsPanel.TabIndex = 0;
			// 
			// toolStrip1
			// 
			this.toolStrip1.Dock = System.Windows.Forms.DockStyle.Bottom;
			this.toolStrip1.ImageScalingSize = new System.Drawing.Size(20, 20);
			this.toolStrip1.Items.AddRange(new System.Windows.Forms.ToolStripItem[] {
            this.SendRequestButton,
            this.Confirm,
            this.Cancel,
            this.Status});
			this.toolStrip1.Location = new System.Drawing.Point(0, 423);
			this.toolStrip1.Name = "toolStrip1";
			this.toolStrip1.Size = new System.Drawing.Size(800, 27);
			this.toolStrip1.TabIndex = 1;
			this.toolStrip1.Text = "toolStrip1";
			// 
			// SendRequestButton
			// 
			this.SendRequestButton.DisplayStyle = System.Windows.Forms.ToolStripItemDisplayStyle.Text;
			this.SendRequestButton.Image = ((System.Drawing.Image)(resources.GetObject("SendRequestButton.Image")));
			this.SendRequestButton.ImageTransparentColor = System.Drawing.Color.Magenta;
			this.SendRequestButton.Name = "SendRequestButton";
			this.SendRequestButton.Size = new System.Drawing.Size(99, 24);
			this.SendRequestButton.Text = "Send request";
			this.SendRequestButton.Click += new System.EventHandler(this.SendRequestButton_Click);
			// 
			// Confirm
			// 
			this.Confirm.DisplayStyle = System.Windows.Forms.ToolStripItemDisplayStyle.Text;
			this.Confirm.Image = ((System.Drawing.Image)(resources.GetObject("Confirm.Image")));
			this.Confirm.ImageTransparentColor = System.Drawing.Color.Magenta;
			this.Confirm.Name = "Confirm";
			this.Confirm.RightToLeft = System.Windows.Forms.RightToLeft.No;
			this.Confirm.Size = new System.Drawing.Size(133, 24);
			this.Confirm.Text = "Confirm and close";
			this.Confirm.Click += new System.EventHandler(this.Confirm_Click);
			// 
			// Cancel
			// 
			this.Cancel.Alignment = System.Windows.Forms.ToolStripItemAlignment.Right;
			this.Cancel.DisplayStyle = System.Windows.Forms.ToolStripItemDisplayStyle.Text;
			this.Cancel.Image = ((System.Drawing.Image)(resources.GetObject("Cancel.Image")));
			this.Cancel.ImageTransparentColor = System.Drawing.Color.Magenta;
			this.Cancel.Margin = new System.Windows.Forms.Padding(0, 1, 1, 1);
			this.Cancel.Name = "Cancel";
			this.Cancel.RightToLeft = System.Windows.Forms.RightToLeft.No;
			this.Cancel.Size = new System.Drawing.Size(57, 25);
			this.Cancel.Text = "Cancel";
			this.Cancel.Click += new System.EventHandler(this.Cancel_Click);
			// 
			// Status
			// 
			this.Status.Alignment = System.Windows.Forms.ToolStripItemAlignment.Right;
			this.Status.BackColor = System.Drawing.SystemColors.Control;
			this.Status.BorderStyle = System.Windows.Forms.BorderStyle.None;
			this.Status.Enabled = false;
			this.Status.Font = new System.Drawing.Font("Segoe UI", 9F);
			this.Status.Name = "Status";
			this.Status.ReadOnly = true;
			this.Status.RightToLeft = System.Windows.Forms.RightToLeft.No;
			this.Status.Size = new System.Drawing.Size(300, 27);
			this.Status.TextBoxTextAlign = System.Windows.Forms.HorizontalAlignment.Right;
			// 
			// FloorplanQueryForm
			// 
			this.AutoScaleDimensions = new System.Drawing.SizeF(8F, 16F);
			this.AutoScaleMode = System.Windows.Forms.AutoScaleMode.Font;
			this.ClientSize = new System.Drawing.Size(800, 450);
			this.Controls.Add(this.toolStrip1);
			this.Controls.Add(this.VariantsPanel);
			this.Name = "FloorplanQueryForm";
			this.Text = "Floorplanquery";
			this.Load += new System.EventHandler(this.FloorplanQueryForm_Load);
			this.toolStrip1.ResumeLayout(false);
			this.toolStrip1.PerformLayout();
			this.ResumeLayout(false);
			this.PerformLayout();

        }

        #endregion

        private System.Windows.Forms.FlowLayoutPanel VariantsPanel;
        private System.Windows.Forms.ToolStrip toolStrip1;
        private System.Windows.Forms.ToolStripButton SendRequestButton;
		private System.Windows.Forms.ToolStripButton Confirm;
        private System.Windows.Forms.ToolStripButton Cancel;
		private System.Windows.Forms.ToolStripTextBox Status;
	}
}