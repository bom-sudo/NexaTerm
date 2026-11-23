; NexaTerm NSIS hooks (included by the Tauri NSIS template).
;
; The template's "Delete the application data" checkbox removes $APPDATA\<bundle identifier>,
; but NexaTerm stores its data in $APPDATA\NexaTerm. Remove that folder too — only when the user
; ticked the checkbox and never during an update. Saved passwords live in Windows Credential
; Manager and are not touched by the uninstaller.

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$APPDATA\NexaTerm"
  ${EndIf}
!macroend
