# 7P-L1 Windows synthetic probe installer (NOT CoSoS product)

This proof-of-feasibility package exists only to measure the WebView2 adapter's
Windows distribution. It is unsigned, requires an installed Evergreen WebView2
runtime, disables real OS account SSO, and installs *only* to D:. Never ship it
as the production CoSoS desktop application.

Build on Windows with .NET 8 SDK and the real bundled renderer. Run
`npm run build --prefix probes/7p-b/renderer`, then `dotnet publish
probes/7p-b/WindowsHost/WindowsHost.csproj -c Release -r win-x64
--self-contained true /p:PublishSingleFile=true /p:DebugType=None
/p:DebugSymbols=false -o D:\projects\CoS-Rust\.test-tmp\7p-l1\installer\publish`.
Set `TMP`, `TEMP`, `DOTNET_CLI_HOME`, and `NUGET_PACKAGES` to D:-rooted
directories before these commands. Then run `build-installer.ps1` to package
the published binary, synthetic extension/renderer, SHA-256 manifest, ZIP
fallback and offline setup EXE compiled
with the Windows .NET Framework C# compiler. (IExpress failed even with a tiny
input on the tested Windows build and is not needed by this path.)
The setup uses a non-elevated .NET Framework installer; by default its destination
is `D:\Apps\CoSoS-7PL1-Synthetic-Probe`. Override with
`COS_7PL1_INSTALL_ROOT=D:\...` before running setup, never C:.

After installation, run `WindowsHost.exe --batch --out D:\...` with a unique
D:-rooted output folder. Always test the generated source-verified manifest and
use `uninstall.cmd` for removal. The uninstaller deletes only packaged files;
user-added and modified packaged files remain untouched. Do not bundle development `node_modules`,
browser profile data, machine credentials, or user local files.

**Acceptance:** install/smoke/uninstall and hashes must be recorded separately
from native UI and IME proof. The sample installer **alone** cannot certify
the 7P-L1 physical-Windows browser-feasibility gate; the distinct native
qualifier must verify a fresh installed host, complete WebView2 checks and
source-bound size/resource measurements together. The combined synthetic
feasibility receipt still cannot certify product MSI, signing, corporate
policy, production startup/private RAM or browser runtime installation.
Use a new `-OutputRoot D:\...` for every packaging run, since the builder refuses
to overwrite existing output.
