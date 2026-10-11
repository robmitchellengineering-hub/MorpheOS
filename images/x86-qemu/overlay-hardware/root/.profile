# Printed on every login to a local console.
#
# Not decoration: this image's whole reason to exist is that a machine with a broken screen can
# be used and diagnosed by a person, and "did a login actually complete?" is otherwise
# indistinguishable from "a getty printed a banner and is now waiting for a password nobody can
# type". The boot check asserts this line appears when nothing was typed.
if [ -t 0 ]; then
    status=$(/usr/bin/morph-boot status 2>/dev/null || echo '{"error":"morph-boot status failed"}')
    printf '\nMORPHEOS CONSOLE READY - %s\n\n' "$status"
fi
