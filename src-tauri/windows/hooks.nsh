; Extra steps for the Windows installer (Tauri's NSIS template calls these
; macros; see bundle.windows.nsis.installerHooks in tauri.conf.json).

; After an uninstall: remove the app's Launch at login registration.
;
; The app registers itself, for the current user, as a value named after the
; product in the Run key. When the user disables that entry in Task Manager,
; Windows keeps a second value of the same name under StartupApproved\Run.
; Tauri's own template removes the Run value in an ordinary uninstall; it is
; named here as well so that this does not rest on the template, and the
; StartupApproved mark is removed nowhere else. Deleting a value that is not
; there does nothing.
;
; An update runs the old version's uninstaller first ($UpdateMode = 1); the
; registration must survive that.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
  ${EndIf}
!macroend
