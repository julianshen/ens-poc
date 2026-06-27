<#
.SYNOPSIS
    Create a Start Menu shortcut that carries an AppUserModelID (AUMID).

.DESCRIPTION
    Windows requires an *unpackaged* Win32 app to have a Start Menu shortcut
    whose System.AppUserModel.ID property matches the AUMID it raises toasts
    under. The registry AppUserModelId key alone (DisplayName/IconUri) is not
    enough — without this shortcut, ToastNotification.Show() succeeds but the
    toast is silently dropped.

    install.ps1 calls this so the installed service can actually raise toasts.
#>
param(
    [Parameter(Mandatory)][string]$ShortcutPath,
    [Parameter(Mandatory)][string]$TargetPath,
    [Parameter(Mandatory)][string]$Aumid,
    [string]$Arguments = "",
    [string]$IconPath = ""
)

$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace TNS {
  [ComImport, Guid("00021401-0000-0000-C000-000000000046")]
  internal class CShellLink {}

  [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown),
   Guid("000214F9-0000-0000-C000-000000000046")]
  internal interface IShellLinkW {
    void GetPath([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder f, int c, IntPtr fd, uint fl);
    void GetIDList(out IntPtr ppidl);
    void SetIDList(IntPtr pidl);
    void GetDescription([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder n, int c);
    void SetDescription([MarshalAs(UnmanagedType.LPWStr)] string n);
    void GetWorkingDirectory([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder d, int c);
    void SetWorkingDirectory([MarshalAs(UnmanagedType.LPWStr)] string d);
    void GetArguments([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder a, int c);
    void SetArguments([MarshalAs(UnmanagedType.LPWStr)] string a);
    void GetHotkey(out short w);
    void SetHotkey(short w);
    void GetShowCmd(out int c);
    void SetShowCmd(int c);
    void GetIconLocation([Out, MarshalAs(UnmanagedType.LPWStr)] StringBuilder p, int c, out int i);
    void SetIconLocation([MarshalAs(UnmanagedType.LPWStr)] string p, int i);
    void SetRelativePath([MarshalAs(UnmanagedType.LPWStr)] string p, uint r);
    void Resolve(IntPtr hwnd, uint fl);
    void SetPath([MarshalAs(UnmanagedType.LPWStr)] string f);
  }

  [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown),
   Guid("0000010b-0000-0000-C000-000000000046")]
  internal interface IPersistFile {
    void GetClassID(out Guid id);
    [PreserveSig] int IsDirty();
    void Load([MarshalAs(UnmanagedType.LPWStr)] string f, uint m);
    void Save([MarshalAs(UnmanagedType.LPWStr)] string f, [MarshalAs(UnmanagedType.Bool)] bool r);
    void SaveCompleted([MarshalAs(UnmanagedType.LPWStr)] string f);
    void GetCurFile([MarshalAs(UnmanagedType.LPWStr)] out string f);
  }

  [StructLayout(LayoutKind.Sequential)]
  internal struct PROPERTYKEY { public Guid fmtid; public uint pid; }

  [StructLayout(LayoutKind.Explicit)]
  internal struct PROPVARIANT {
    [FieldOffset(0)] public ushort vt;
    [FieldOffset(8)] public IntPtr p;
  }

  [ComImport, InterfaceType(ComInterfaceType.InterfaceIsIUnknown),
   Guid("886d8eeb-8cf2-4446-8d02-cdba1dbdcf99")]
  internal interface IPropertyStore {
    void GetCount(out uint c);
    void GetAt(uint i, out PROPERTYKEY k);
    void GetValue(ref PROPERTYKEY k, out PROPVARIANT v);
    void SetValue(ref PROPERTYKEY k, ref PROPVARIANT v);
    void Commit();
  }

  public static class AumidShortcut {
    public static void Create(string shortcutPath, string target, string args, string icon, string aumid) {
      var link = (IShellLinkW)new CShellLink();
      link.SetPath(target);
      if (!string.IsNullOrEmpty(args)) link.SetArguments(args);
      if (!string.IsNullOrEmpty(icon)) link.SetIconLocation(icon, 0);

      var store = (IPropertyStore)link;
      // PKEY_AppUserModel_ID = {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}, pid 5
      var key = new PROPERTYKEY { fmtid = new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), pid = 5 };
      var pv = new PROPVARIANT { vt = 31 /* VT_LPWSTR */, p = Marshal.StringToCoTaskMemUni(aumid) };
      store.SetValue(ref key, ref pv);
      store.Commit();
      Marshal.FreeCoTaskMem(pv.p);

      ((IPersistFile)link).Save(shortcutPath, true);
    }
  }
}
"@

[TNS.AumidShortcut]::Create($ShortcutPath, $TargetPath, $Arguments, $IconPath, $Aumid)
Write-Host "Created shortcut '$ShortcutPath' with AUMID '$Aumid'."
