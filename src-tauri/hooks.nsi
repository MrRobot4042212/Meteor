!macro NSIS_HOOK_POSTINSTALL
  MessageBox MB_YESNO "Para que METEOR pueda medir las estadísticas de rendimiento de tus videojuegos (como los FPS o la temperatura), necesita ejecutarse con permisos de Administrador.$\n$\n¿Deseas configurar METEOR para que siempre se abra como Administrador de forma permanente?" IDNO skip_admin
    ; Si el usuario dice que sí (IDYES), escribimos la clave de registro que fuerza el UAC en Windows
    WriteRegStr HKCU "Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers" "$INSTDIR\Meteor.exe" "~ RUNASADMIN"
  skip_admin:
!macroend
