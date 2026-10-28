
#ifndef AppVersion
  #error Pass the version with /DAppVersion=x.y.z
#endif
#ifndef ExeName
  #error Pass the name of the executable with /DExeName=name.exe
#endif
#ifndef SourceExe
  #error Pass the built executable with /DSourceExe=path
#endif

; Keep this ID stable so upgrades find the existing installation.
#define AppGuid "{B4D1E7A2-5C3F-4F68-9A0E-2D7C61B8E4F3}"

[Setup]
AppId={{B4D1E7A2-5C3F-4F68-9A0E-2D7C61B8E4F3}
AppName=Notch
AppVersion={#AppVersion}
AppPublisher=hibatillah
AppPublisherURL=https://github.com/hibatillah/notch
DefaultDirName={localappdata}\Programs\Notch
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
LicenseFile=..\LICENSE
SetupIconFile=..\assets\branding\logo.ico
WizardSmallImageFile=..\assets\branding\logo.png
UninstallDisplayIcon={app}\{#ExeName}
CloseApplications=force
RestartApplications=no

[Tasks]
Name: "startup"; Description: "Start Notch when Windows starts"; GroupDescription: "Options:"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "{#ExeName}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion
Source: "..\assets\fonts\Geist-LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Notch"; Filename: "{app}\{#ExeName}"; IconFilename: "{app}\{#ExeName}"; IconIndex: 0

[Registry]
; The same entry the "Start with Windows" item in the notch's tray menu switches.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Notch"; ValueData: """{app}\{#ExeName}"""; Flags: uninsdeletevalue; Tasks: startup

[Run]
Filename: "{app}\{#ExeName}"; Description: "Start Notch"; Flags: nowait postinstall skipifsilent

[Code]
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#AppGuid}_is1';

{ Asks before replacing a copy that is already installed, saying which versions are involved. }
function InitializeSetup(): Boolean;
var
  Installed: String;
  InstalledVersion, NewVersion: Int64;
  Question: String;
begin
  Result := True;
  if not RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', Installed) then
    Exit;
  if StrToVersion(Installed, InstalledVersion) and StrToVersion('{#AppVersion}', NewVersion) then
  begin
    if ComparePackedVersion(InstalledVersion, NewVersion) < 0 then
      Question := 'Notch ' + Installed + ' is already installed.' + #13#10 + 'Update it to version {#AppVersion}?'
    else if ComparePackedVersion(InstalledVersion, NewVersion) = 0 then
      Question := 'Notch ' + Installed + ' is already installed.' + #13#10 + 'Install it again?'
    else
      Question := 'A newer Notch (' + Installed + ') is already installed.' + #13#10 + 'Replace it with version {#AppVersion}?';
  end
  else
    Question := 'Notch ' + Installed + ' is already installed.' + #13#10 + 'Replace it with version {#AppVersion}?';
  Result := SuppressibleMsgBox(Question, mbConfirmation, MB_YESNO, IDYES) = IDYES;
end;
