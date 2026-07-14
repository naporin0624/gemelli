; Inno Setup script for gemelli (Windows x64).
; Build with: ISCC.exe /DMyAppVersion=X.Y.Z /DSourceDir=<staged> /DOutputDir=<out> gemelli.iss
; Spout/Syphon are compiled in; no external runtime is required.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "."
#endif
#ifndef OutputDir
  #define OutputDir "dist"
#endif

#define MyAppName "gemelli"
#define MyAppPublisher "naporin0624"
#define MyAppExeName "gemelli-gui.exe"
#define MyAppIco SourceDir + "\icon.ico"

[Setup]
; A stable AppId keeps upgrades/uninstall consistent across versions.
AppId={{3FDAFA9C-6113-4555-8F3A-8FE7E9FFD465}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=gemelli-{#MyAppVersion}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile={#MyAppIco}
UninstallDisplayIcon={app}\icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\gemelli.exe";          DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\gemelli-gui.exe";      DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\icon.ico";             DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\README.md";            DestDir: "{app}"; Flags: ignoreversion isreadme
Source: "{#SourceDir}\THIRD-PARTY-NOTICES";  DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}";           Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}";     Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall skipifsilent
