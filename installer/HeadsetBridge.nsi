; HeadsetBridge für Xbox – Setup (NSIS)
; Wird von Build.ps1 aufgerufen:
;   makensis -DVERSION=2.0.0 -DAPP_DIR=<publish-Ordner> -DOUTPUT_FILE=<Setup.exe> HeadsetBridge.nsi
; APP_DIR enthält die veröffentlichte WinUI-App samt engine\ und Lizenzhinweise.txt.
; Build.ps1 erzeugt zusätzlich uninstall-files.nsh mit allen installierten Dateien,
; damit die Deinstallation nur eigene Dateien entfernt.

Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "2.0.0"
!endif
!ifndef APP_DIR
  !error "APP_DIR fehlt"
!endif
!ifndef OUTPUT_FILE
  !define OUTPUT_FILE "HeadsetBridge-Setup.exe"
!endif

!define REG_KEY "Software\HeadsetBridgeDesktop"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\HeadsetBridgeDesktop"

Name "HeadsetBridge"
OutFile "${OUTPUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\HeadsetBridge"
InstallDirRegKey HKCU "${REG_KEY}" "InstallDir"
BrandingText "HeadsetBridge ${VERSION}"
Icon "..\src\HeadsetBridge.App\Assets\headsetbridge.ico"
UninstallIcon "..\src\HeadsetBridge.App\Assets\headsetbridge.ico"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=0 "ProductName" "HeadsetBridge"
VIAddVersionKey /LANG=0 "FileDescription" "HeadsetBridge Setup"
VIAddVersionKey /LANG=0 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=0 "LegalCopyright" "HeadsetBridge contributors · MIT"

!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"

!define MUI_ICON "..\src\HeadsetBridge.App\Assets\headsetbridge.ico"
!define MUI_UNICON "..\src\HeadsetBridge.App\Assets\headsetbridge.ico"
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "$(WelcomeTitle)"
!define MUI_WELCOMEPAGE_TEXT "$(WelcomeText)"
!define MUI_FINISHPAGE_TITLE "$(FinishTitle)"
!define MUI_FINISHPAGE_TEXT "$(FinishText)"
!define MUI_FINISHPAGE_RUN "$INSTDIR\HeadsetBridge.exe"
!define MUI_FINISHPAGE_RUN_TEXT "$(RunText)"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Die Setup-Sprache folgt Windows: Deutsch für deutsche Systeme, sonst Englisch.
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "German"

LangString WelcomeTitle ${LANG_GERMAN} "HeadsetBridge für Xbox einrichten"
LangString WelcomeTitle ${LANG_ENGLISH} "Set up HeadsetBridge for Xbox"
LangString WelcomeText ${LANG_GERMAN} "Xbox-Audio auf deinem PC – über das Windows-Ausgabegerät deiner Wahl.$\r$\n$\r$\nDie Installation gilt nur für dein Windows-Konto und benötigt keine weiteren Laufzeitumgebungen."
LangString WelcomeText ${LANG_ENGLISH} "Xbox audio on your PC – through the Windows output device of your choice.$\r$\n$\r$\nThe installation is for your Windows account only and needs no additional runtimes."
LangString FinishTitle ${LANG_GERMAN} "HeadsetBridge ist eingerichtet"
LangString FinishTitle ${LANG_ENGLISH} "HeadsetBridge is ready"
LangString FinishText ${LANG_GERMAN} "Starte HeadsetBridge und klicke auf „Mit Xbox verbinden“. Beim ersten Mal meldest du dich mit dem Microsoft-Konto deiner Xbox an."
LangString FinishText ${LANG_ENGLISH} "Start HeadsetBridge and click “Connect to Xbox”. The first time, sign in with the Microsoft account of your Xbox."
LangString RunText ${LANG_GERMAN} "HeadsetBridge starten"
LangString RunText ${LANG_ENGLISH} "Start HeadsetBridge"
LangString Need64 ${LANG_GERMAN} "HeadsetBridge benötigt Windows 10 oder 11 (64 Bit)."
LangString Need64 ${LANG_ENGLISH} "HeadsetBridge requires Windows 10 or 11 (64-bit)."
LangString CloseApp ${LANG_GERMAN} "Bitte schließe HeadsetBridge, bevor du fortfährst."
LangString CloseApp ${LANG_ENGLISH} "Please close HeadsetBridge before continuing."
LangString UninstallName ${LANG_GERMAN} "Deinstallieren"
LangString UninstallName ${LANG_ENGLISH} "Uninstall"

!macro CheckRunning
  FindWindow $0 "" "HeadsetBridge für Xbox"
  ${If} $0 == 0
    FindWindow $0 "" "HeadsetBridge for Xbox"
  ${EndIf}
  ${If} $0 == 0
    FindWindow $0 "HeadsetBridgeDesktop100"   ; Version 1.x
  ${EndIf}
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONINFORMATION "$(CloseApp)"
    Abort
  ${EndIf}
!macroend

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "$(Need64)"
    Abort
  ${EndIf}
  SetRegView 64
  SetShellVarContext current
  !insertmacro CheckRunning
FunctionEnd

Section "HeadsetBridge"
  SetOutPath "$INSTDIR"

  ; Reste von Version 1.x (Launcher, entpackter WebView2-Host) entfernen.
  Delete "$INSTDIR\Deinstallieren.exe"
  Delete "$INSTDIR\Anleitung.txt"
  Delete "$LOCALAPPDATA\HeadsetBridge\desktop-1.2.0\HeadsetBridge.Desktop.exe"
  Delete "$LOCALAPPDATA\HeadsetBridge\desktop-1.2.0\WebView2Loader.dll"
  RMDir "$LOCALAPPDATA\HeadsetBridge\desktop-1.2.0"
  Delete "$LOCALAPPDATA\HeadsetBridge\engine-1.0.0\xbox-solo-audio.exe"
  Delete "$LOCALAPPDATA\HeadsetBridge\engine-1.0.0\Anleitung.txt"
  Delete "$LOCALAPPDATA\HeadsetBridge\engine-1.0.0\Lizenzhinweise.txt"
  RMDir "$LOCALAPPDATA\HeadsetBridge\engine-1.0.0"
  RMDir /r "$LOCALAPPDATA\HeadsetBridge\WebView"

  File /r "${APP_DIR}\*.*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\HeadsetBridge"
  CreateShortcut "$SMPROGRAMS\HeadsetBridge\HeadsetBridge.lnk" "$INSTDIR\HeadsetBridge.exe"
  CreateShortcut "$SMPROGRAMS\HeadsetBridge\$(UninstallName).lnk" "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\HeadsetBridge\Anleitung.lnk"
  Delete "$SMPROGRAMS\HeadsetBridge\Deinstallieren.lnk"
  CreateShortcut "$DESKTOP\HeadsetBridge.lnk" "$INSTDIR\HeadsetBridge.exe"

  WriteRegStr HKCU "${REG_KEY}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${REG_KEY}" "Language" "$LANGUAGE"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "HeadsetBridge"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "HeadsetBridge contributors"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\HeadsetBridge.exe,0"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/MGPersonalProfile/headsetbridge-for-xbox"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
SectionEnd

Function un.onInit
  SetRegView 64
  SetShellVarContext current
  ReadRegStr $LANGUAGE HKCU "${REG_KEY}" "Language"
  !insertmacro CheckRunning
FunctionEnd

Section "Uninstall"
  ; Nur die von diesem Setup installierten Dateien (Liste von Build.ps1).
  !include "uninstall-files.nsh"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$DESKTOP\HeadsetBridge.lnk"
  Delete "$SMPROGRAMS\HeadsetBridge\HeadsetBridge.lnk"
  Delete "$SMPROGRAMS\HeadsetBridge\Deinstallieren.lnk"
  Delete "$SMPROGRAMS\HeadsetBridge\Uninstall.lnk"
  RMDir "$SMPROGRAMS\HeadsetBridge"

  ; Einstellungen der App; die Xbox-Anmeldung bleibt bewusst erhalten.
  Delete "$LOCALAPPDATA\HeadsetBridge\settings.json"
  RMDir "$LOCALAPPDATA\HeadsetBridge"

  DeleteRegKey HKCU "${REG_KEY}"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
