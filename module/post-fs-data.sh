MODDIR=${0%/*}
chmod 0755 ${MODDIR}/prop
 ${MODDIR}/prop
resetprop -c 2>/dev/null || true