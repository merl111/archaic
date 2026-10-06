Unicode true
!include "MUI2.nsh"
!define MUI_ICON "${STAGE}\archaic.ico"
!define MUI_UNICON "${STAGE}\archaic.ico"
Name "Archaic"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\Archaic"
RequestExecutionLevel user
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
Section "Archaic"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\Archaic"
  CreateShortcut "$SMPROGRAMS\Archaic\Archaic.lnk" "$INSTDIR\archaic.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "DisplayName" "Archaic"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "DisplayIcon" '$"$INSTDIR\archaic.exe$",0'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
  WriteRegStr HKCU "Software\Classes\Archaic.Matrix" "" "Matrix link"
  WriteRegStr HKCU "Software\Classes\Archaic.Matrix" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\Archaic.Matrix\shell\open\command" "" '$"$INSTDIR\archaic.exe$" --open $"%1$"'
  WriteRegStr HKCU "Software\Archaic\Capabilities" "ApplicationName" "Archaic"
  WriteRegStr HKCU "Software\Archaic\Capabilities\URLAssociations" "matrix" "Archaic.Matrix"
  WriteRegStr HKCU "Software\RegisteredApplications" "Archaic" "Software\Archaic\Capabilities"
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic" "InstallLocation"
  StrCmp $0 "$INSTDIR" 0 keep_registration
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Archaic"
  DeleteRegKey HKCU "Software\Classes\Archaic.Matrix"
  DeleteRegKey HKCU "Software\Archaic\Capabilities"
  DeleteRegValue HKCU "Software\RegisteredApplications" "Archaic"
  keep_registration:
  Delete "$SMPROGRAMS\Archaic\Archaic.lnk"
  RMDir "$SMPROGRAMS\Archaic"
  Delete "$INSTDIR\archaic.exe"
  Delete "$INSTDIR\archaic.ico"
  Delete "$INSTDIR\cui.dll"
  Delete "$INSTDIR\Microsoft.WindowsAppRuntime.Bootstrap.dll"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\build.json"
  RMDir /r "$INSTDIR\licenses"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  ; Account data and credential-vault entries are deliberately retained.
SectionEnd
