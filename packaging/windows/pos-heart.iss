; POS Heart installer for Windows (Inno Setup 6).
;   cargo build --release --locked -p pos-desktop --bin pos-heart
;   iscc /DVersion=0.1.0 packaging\windows\pos-heart.iss
; Installs per user by default (no administrator needed); shop data lives in
; the user's app-data folder and is kept on uninstall and on updates.
#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{538A2A04-8A7E-4EE9-958D-67C3452AC810}
AppName=POS Heart
AppVersion={#Version}
AppVerName=POS Heart {#Version}
AppPublisher=POS Heart
DefaultDirName={autopf}\POS Heart
DefaultGroupName=POS Heart
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\icon\icon.ico
UninstallDisplayIcon={app}\pos-heart.exe
OutputDir=..\..\target\dist
OutputBaseFilename=POS-Heart-{#Version}-Windows-Setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; An update closes a running POS Heart first.
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "..\..\target\release\pos-heart.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\POS Heart"; Filename: "{app}\pos-heart.exe"
Name: "{group}\POS Heart (Demo)"; Filename: "{app}\pos-heart.exe"; Parameters: "--demo"
Name: "{autodesktop}\POS Heart"; Filename: "{app}\pos-heart.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\pos-heart.exe"; Description: "{cm:LaunchProgram,POS Heart}"; Flags: nowait postinstall skipifsilent
