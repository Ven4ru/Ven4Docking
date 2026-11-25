using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

namespace Ven4Docking.Core.Apps;

/// <summary>
/// Extracts a program icon (exe, .lnk, .url, any file) as a PNG with alpha via the shell's
/// IShellItemImageFactory — the same image Explorer shows, up to 256 px.
/// </summary>
public static class IconExtractor
{
    /// <summary>Writes <paramref name="pngPath"/>; returns false if the shell has no image for the file.</summary>
    public static bool TryExtractPng(string sourcePath, string pngPath, int size = 128)
    {
        if (!OperatingSystem.IsWindows()) return false;
        IntPtr hbmp = IntPtr.Zero;
        try
        {
            Guid iid = typeof(IShellItemImageFactory).GUID;
            int hr = SHCreateItemFromParsingName(sourcePath, IntPtr.Zero, ref iid, out var factory);
            if (hr != 0 || factory is null) return false;
            try
            {
                hr = factory.GetImage(new NativeSize { cx = size, cy = size }, SIIGBF.BiggerSizeOk | SIIGBF.IconOnly, out hbmp);
                if (hr != 0 || hbmp == IntPtr.Zero)
                    hr = factory.GetImage(new NativeSize { cx = size, cy = size }, SIIGBF.BiggerSizeOk, out hbmp);
                if (hr != 0 || hbmp == IntPtr.Zero) return false;
            }
            finally { Marshal.ReleaseComObject(factory); }

            using var bmp = FromHBitmapWithAlpha(hbmp);
            if (bmp is null) return false;
            Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(pngPath))!);
            bmp.Save(pngPath, ImageFormat.Png);
            return true;
        }
        catch (COMException) { return false; }
        catch (ExternalException) { return false; }
        finally
        {
            if (hbmp != IntPtr.Zero) DeleteObject(hbmp);
        }
    }

    /// <summary>GDI+ drops alpha in Image.FromHbitmap; copy the 32-bit pixels (premultiplied BGRA) by hand.</summary>
    private static Bitmap? FromHBitmapWithAlpha(IntPtr hbmp)
    {
        if (GetObject(hbmp, Marshal.SizeOf<BITMAP>(), out BITMAP info) == 0) return null;
        if (info.bmBitsPixel != 32 || info.bmBits == IntPtr.Zero)
            return Image.FromHbitmap(hbmp);

        int w = info.bmWidth, h = Math.Abs(info.bmHeight);
        var result = new Bitmap(w, h, PixelFormat.Format32bppPArgb);
        var data = result.LockBits(new Rectangle(0, 0, w, h), ImageLockMode.WriteOnly, PixelFormat.Format32bppPArgb);
        try
        {
            int stride = info.bmWidthBytes;
            var row = new byte[stride];
            for (int y = 0; y < h; y++)
            {
                // DIB sections from the shell are bottom-up when bmHeight > 0.
                int srcY = info.bmHeight > 0 ? h - 1 - y : y;
                Marshal.Copy(info.bmBits + srcY * stride, row, 0, stride);
                Marshal.Copy(row, 0, data.Scan0 + y * data.Stride, Math.Min(stride, data.Stride));
            }
        }
        finally { result.UnlockBits(data); }
        return result;
    }

    [Flags]
    private enum SIIGBF
    {
        ResizeToFit = 0x00,
        BiggerSizeOk = 0x01,
        IconOnly = 0x04,
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeSize
    {
        public int cx;
        public int cy;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct BITMAP
    {
        public int bmType;
        public int bmWidth;
        public int bmHeight;
        public int bmWidthBytes;
        public ushort bmPlanes;
        public ushort bmBitsPixel;
        public IntPtr bmBits;
    }

    [ComImport, Guid("bcc18b79-ba16-442f-80c4-8a59c30c463b"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    private interface IShellItemImageFactory
    {
        [PreserveSig]
        int GetImage(NativeSize size, SIIGBF flags, out IntPtr phbm);
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode, PreserveSig = true)]
    private static extern int SHCreateItemFromParsingName(string pszPath, IntPtr pbc, ref Guid riid,
        [MarshalAs(UnmanagedType.Interface)] out IShellItemImageFactory ppv);

    [DllImport("gdi32.dll")]
    private static extern int GetObject(IntPtr h, int c, out BITMAP pv);

    [DllImport("gdi32.dll")]
    private static extern bool DeleteObject(IntPtr ho);
}
