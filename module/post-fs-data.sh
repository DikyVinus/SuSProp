#!/system/bin/sh

MODDIR=${0%/*}

# detect primary ABI
ABI="$(getprop ro.product.cpu.abi)"

case "$ABI" in
    arm64-v8a)
        BIN="prop64"
        ;;
    armeabi-v7a|armeabi)
        BIN="prop32"
        ;;
    *)
        # fallback: prefer 64 if present
        if [ -x "$MODDIR/prop64" ]; then
            BIN="prop64"
        else
            BIN="prop32"
        fi
        ;;
esac

# ensure executable
chmod 0755 "$MODDIR/$BIN"

# run selected binary
"$MODDIR/$BIN"

# clear property cache
resetprop -c 2>/dev/null || true