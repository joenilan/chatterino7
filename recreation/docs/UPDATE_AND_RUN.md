# Update and run on Windows

Close Jawjack, then run from PowerShell:

```powershell
& E:\git\chatterino7\recreation\update-and-run.ps1
```

The script fast-forwards `recreation/gpui-foundation`, builds the locked release
with your user-local Rustup and Rust 1.99.0, then launches the executable. It stops
on a different branch, uncommitted work, an open executable from this checkout,
or a failed pull/build. It does not install tools, change execution policy, kill
processes, discard edits or run GitHub workflows.

It uses the original development preview profile when its saved workspace exists;
otherwise it uses normal local application data. To choose an existing profile:

```powershell
.\update-and-run.ps1 -ProfileRoot 'C:\path\to\existing-profile'
```

The chosen root contains `ChatWorkbench\workspace.json`. No profile is copied or
migrated. Environment changes apply only to the launched process and are restored
in the calling shell. Credentials continue to use the application's OS vault.

This launcher has been source-reviewed; Windows execution is pending. It does
not claim that a launched window has passed a live UI review.
