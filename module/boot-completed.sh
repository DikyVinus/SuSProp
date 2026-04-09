#!/system/bin/sh

# runs in boot-completed.sh → no wait needed

resetprop | while IFS='[]' read -r _ k _ v _; do
    case "$k" in
        ro.lineage.*|sys.lineage_*)
            resetprop -d "$k"
        ;;
        *pihook*|*pixelprops*)
            resetprop -d "$k"
        ;;
    esac
done

resetprop -c 2>/dev/null || true
