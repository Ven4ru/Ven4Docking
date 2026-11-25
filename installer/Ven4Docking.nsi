; Ven4Docking — установщик (NSIS 3, Unicode).
; Ставится на пользователя без прав администратора в %LOCALAPPDATA%\Programs\Ven4Docking.
; Права администратора просит только отдельный шаг — правило брандмауэра «Ven4Docking»
; (TCP 8765 + UDP 8766, только локальная подсеть). Отказ — установка всё равно успешна.
;
; Сборка: build.ps1 (локально) или .github/workflows/build.yml:
;   makensis /DVERSION=1.0.0 /DSTAGE=<папка с файлами> /DOUTFILE=<путь к Setup.exe> installer\Ven4Docking.nsi
; STAGE: ven4docking-agent.exe, Ven4Docking.exe + файлы редактора, panel\, apk\ven4docking.apk + latest.json.

Unicode true
ManifestDPIAware true
SetCompressor /SOLID lzma

!ifndef VERSION
  !define VERSION "1.0.0"
!endif
!ifndef STAGE
  !define STAGE "..\dist\stage"
!endif
!ifndef OUTFILE
  !define OUTFILE "..\dist\Ven4Docking-Setup-${VERSION}.exe"
!endif

!define APP "Ven4Docking"
!define AGENT_EXE "ven4docking-agent.exe"
!define EDITOR_EXE "Ven4Docking.exe"
!define FW_RULE "Ven4Docking"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Ven4Docking"
!define ICON "..\assets\icon\ven4docking.ico"

Name "${APP}"
Caption "${APP} ${VERSION} — установка"
UninstallCaption "${APP} — удаление"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP}"
RequestExecutionLevel user
BrandingText "${APP} ${VERSION} · Ven4verse"
ShowInstDetails show
ShowUninstDetails show

VIProductVersion "${VERSION}.0"
VIFileVersion "${VERSION}.0"
VIAddVersionKey /LANG=1049 "ProductName" "${APP}"
VIAddVersionKey /LANG=1049 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1049 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1049 "FileDescription" "Установщик ${APP}"
VIAddVersionKey /LANG=1049 "CompanyName" "Ven4ru"
VIAddVersionKey /LANG=1049 "LegalCopyright" "MIT, Ven4ru"

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!define MUI_ICON "${ICON}"
!define MUI_UNICON "${ICON}"
!define MUI_ABORTWARNING

!define MUI_WELCOMEPAGE_TITLE "Ven4Docking ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "Ven4Docking превращает старый Android-телефон в тач-панель вашего ПК: часы, погода, нагрузка, серверы, лимиты нейросетей, запуск приложений.$\r$\n$\r$\nПрограмма ставится только для вас, права администратора не нужны. Их попросят один раз — для правила брандмауэра, чтобы телефон в домашней сети достучался до ПК.$\r$\n$\r$\nНажмите «Далее»."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "Ven4Docking установлен"
!define MUI_FINISHPAGE_TEXT "Агент будет запускаться при входе в Windows (значок в трее).$\r$\n$\r$\nВ редакторе откройте «Телефоны» → «Подключить телефон». По кабелю надёжнее."
!define MUI_FINISHPAGE_RUN
!define MUI_FINISHPAGE_RUN_TEXT "Запустить агент и редактор"
!define MUI_FINISHPAGE_RUN_FUNCTION LaunchApp
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!define MUI_COMPONENTSPAGE_TEXT_TOP "Программа будет удалена. Настройки, раскладки экранов и привязки телефонов хранятся отдельно и по умолчанию остаются — отметьте пункт ниже, чтобы удалить и их."
!define MUI_COMPONENTSPAGE_TEXT_COMPLIST "Что ещё удалить:"
!insertmacro MUI_UNPAGE_COMPONENTS
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Russian"

; ---------------------------------------------------------------------------
; Общие функции установщика и деинсталлятора

; Закрыть запущенные агент и редактор (только процессы этого пользователя).
!macro CLOSE_RUNNING UN
Function ${UN}CloseRunning
  DetailPrint "Закрываю запущенный Ven4Docking…"
  nsExec::Exec '"$SYSDIR\taskkill.exe" /IM "${EDITOR_EXE}"'
  Pop $0
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM "${AGENT_EXE}"'
  Pop $0
  Sleep 1500
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM "${EDITOR_EXE}"'
  Pop $0
FunctionEnd
!macroend
!insertmacro CLOSE_RUNNING ""
!insertmacro CLOSE_RUNNING "un."

; $0 = 1, если правило брандмауэра есть (чтение правил прав не требует).
!macro FW_EXISTS UN
Function ${UN}FirewallRuleExists
  nsExec::Exec '"$SYSDIR\netsh.exe" advfirewall firewall show rule name="${FW_RULE}"'
  Pop $0
  ${If} $0 == 0
    StrCpy $0 1
  ${Else}
    StrCpy $0 0
  ${EndIf}
FunctionEnd
!macroend
!insertmacro FW_EXISTS ""
!insertmacro FW_EXISTS "un."

; ---------------------------------------------------------------------------
; Установка

Section "Ven4Docking" SecMain
  SectionIn RO
  Call CloseRunning

  SetOutPath "$INSTDIR"
  File /r "${STAGE}\*.*"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; автозапуск агента (то же значение, что ставит редактор: «Настройки» → «Автозапуск»)
  WriteRegStr HKCU "${RUN_KEY}" "${APP}" '"$INSTDIR\${AGENT_EXE}"'

  ; меню «Пуск» (на пользователя)
  CreateShortcut "$SMPROGRAMS\${APP}.lnk" "$INSTDIR\${EDITOR_EXE}" "" "$INSTDIR\${EDITOR_EXE}" 0 SW_SHOWNORMAL "" "Ven4Docking — редактор панели телефона"

  ; «Приложения и возможности»
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Ven4ru"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://ven4tools.ru"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" '"$INSTDIR\${EDITOR_EXE}",0'
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

; Отдельный шаг с правами администратора: только правило брандмауэра.
Section "-Firewall" SecFirewall
  Call FirewallRuleExists
  ${If} $0 == 1
    DetailPrint "Правило брандмауэра «${FW_RULE}» уже есть."
    Return
  ${EndIf}

  DetailPrint "Запрашиваю права администратора для правила брандмауэра «${FW_RULE}»…"
  ClearErrors
  ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="${FW_RULE}" >nul & netsh advfirewall firewall add rule name="${FW_RULE}" dir=in action=allow protocol=TCP localport=8765 remoteip=localsubnet profile=any >nul & netsh advfirewall firewall add rule name="${FW_RULE}" dir=in action=allow protocol=UDP localport=8766 remoteip=localsubnet profile=any >nul' SW_HIDE

  Call FirewallRuleExists
  ${If} $0 == 1
    DetailPrint "Правило брандмауэра «${FW_RULE}» добавлено: TCP 8765, UDP 8766, локальная подсеть."
  ${Else}
    DetailPrint "Правило брандмауэра не добавлено (нет прав администратора)."
    MessageBox MB_OK|MB_ICONEXCLAMATION "Правило брандмауэра не добавлено — права администратора не получены.$\r$\n$\r$\nVen4Docking установлен и работает, но телефон в домашней сети, скорее всего, не достучится до ПК (брандмауэр Windows закроет порты 8765 и 8766).$\r$\n$\r$\nЧтобы добавить правило позже, запустите установщик ещё раз и согласитесь на запрос прав." /SD IDOK
  ${EndIf}
SectionEnd

Function LaunchApp
  Exec '"$INSTDIR\${AGENT_EXE}"'
  Exec '"$INSTDIR\${EDITOR_EXE}"'
FunctionEnd

; ---------------------------------------------------------------------------
; Удаление

Function un.onInit
  ; рекурсивное удаление — только своей папки
  ${IfNot} ${FileExists} "$INSTDIR\${AGENT_EXE}"
  ${OrIfNot} ${FileExists} "$INSTDIR\uninstall.exe"
    MessageBox MB_OK|MB_ICONSTOP "Не похоже на папку Ven4Docking: $INSTDIR" /SD IDOK
    Abort
  ${EndIf}
FunctionEnd

Section "-un.Main"
  Call un.CloseRunning

  ; автозапуск — только если он наш
  ReadRegStr $0 HKCU "${RUN_KEY}" "${APP}"
  ${If} $0 == '"$INSTDIR\${AGENT_EXE}"'
  ${OrIf} $0 == "$INSTDIR\${AGENT_EXE}"
    DeleteRegValue HKCU "${RUN_KEY}" "${APP}"
  ${EndIf}

  Delete "$SMPROGRAMS\${APP}.lnk"

  ; программа и её кэш (platform-tools, данные WebView2); настройки — отдельным пунктом
  RMDir /r "$INSTDIR"
  RMDir /r "$LOCALAPPDATA\${APP}"
  DeleteRegKey HKCU "${UNINST_KEY}"

  ; правило брандмауэра — с правами администратора
  Call un.FirewallRuleExists
  ${If} $0 == 1
    DetailPrint "Удаляю правило брандмауэра «${FW_RULE}» (нужны права администратора)…"
    ExecShellWait "runas" "$SYSDIR\cmd.exe" '/c netsh advfirewall firewall delete rule name="${FW_RULE}" >nul' SW_HIDE
    Call un.FirewallRuleExists
    ${If} $0 == 1
      MessageBox MB_OK|MB_ICONEXCLAMATION "Правило брандмауэра «${FW_RULE}» не удалено — права администратора не получены.$\r$\nЕго можно удалить вручную: «Брандмауэр Защитника Windows» → «Дополнительные параметры» → «Правила для входящих подключений»." /SD IDOK
    ${EndIf}
  ${EndIf}
SectionEnd

Section /o "un.Удалить настройки (раскладки, телефоны, серверы — папка %APPDATA%\Ven4Docking)" UnSecSettings
  RMDir /r "$APPDATA\${APP}"
SectionEnd
