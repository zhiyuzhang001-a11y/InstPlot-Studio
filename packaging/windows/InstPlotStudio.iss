#define AppName "InstPlot Studio"
#define AppPublisher "InstPlot Studio contributors"
#define AppExeName "instplot-studio.exe"

[Setup]
AppId={{F5A7E98E-2AFB-4E58-8DF8-C20DB09D42A2}
AppName={#AppName}
AppVersion={#MyVersion}
AppPublisher={#AppPublisher}
DefaultDirName={localappdata}\Programs\InstPlot Studio
DefaultGroupName=InstPlot Studio
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=InstPlot-Studio-{#MyVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
UninstallDisplayName=InstPlot Studio
SetupIconFile={#SourceDir}\InstPlotStudio.ico
UninstallDisplayIcon={app}\{#AppExeName},0
VersionInfoVersion={#MyVersionInfoVersion}

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SourceDir}\instplot-studio.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\InstPlot Studio"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; IconIndex: 0
Name: "{autodesktop}\InstPlot Studio"; Filename: "{app}\{#AppExeName}"; IconFilename: "{app}\{#AppExeName}"; IconIndex: 0; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}"; Description: "Launch InstPlot Studio"; Flags: nowait postinstall skipifsilent
