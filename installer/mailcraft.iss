; MailCraft - Windows installer (Inno Setup 6). Per-user (no admin rights).
; Built by scripts/build-release.ps1, which passes /DAppVersion=<Cargo.toml version>.
; The .msg association itself is done by `mailcraft.exe --register [--default]`, the same code
; as the "Lecteur par défaut" option in the app's settings, and undone by `--unregister`.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "MailCraft"
#define AppExe "mailcraft.exe"

[Setup]
AppId={{B6F3D1A2-8C4E-4F7B-9E21-5A0D3C7B9E64}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Felzow47
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
ChangesAssociations=yes
OutputDir=..\dist
OutputBaseFilename=MailCraft-Setup-{#AppVersion}
SetupIconFile=..\assets\mailcraft.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[CustomMessages]
fr.DefaultGroup=Fichiers Outlook :
en.DefaultGroup=Outlook files:
fr.DefaultApp=Ouvrir les fichiers .msg avec MailCraft (lecteur par défaut)
en.DefaultApp=Open .msg files with MailCraft (default viewer)
fr.Desktop=Créer une icône sur le Bureau
en.Desktop=Create a desktop icon
fr.Launch=Lancer MailCraft
en.Launch=Launch MailCraft

[Tasks]
Name: "defaultapp"; Description: "{cm:DefaultApp}"; GroupDescription: "{cm:DefaultGroup}"
Name: "desktop"; Description: "{cm:Desktop}"; Flags: unchecked

[Files]
Source: "..\target\release\mailcraft.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\NOTICE.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktop

[Run]
; Always listed in "Open with" and Default apps; the default viewer only when the box is ticked.
Filename: "{app}\{#AppExe}"; Parameters: "--register --default"; Flags: runhidden waituntilterminated; Tasks: defaultapp
Filename: "{app}\{#AppExe}"; Parameters: "--register"; Flags: runhidden waituntilterminated; Tasks: not defaultapp
Filename: "{app}\{#AppExe}"; Description: "{cm:Launch}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{app}\{#AppExe}"; Parameters: "--unregister"; Flags: runhidden waituntilterminated; RunOnceId: "UnregisterMsg"
