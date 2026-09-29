#!/usr/bin/env bash
set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
cd "${repository_root}"

if [[ "$(uname -s)" != "Linux" ]]; then
    echo "Linux packages must be built on Linux." >&2
    exit 1
fi

version=$(scripts/read-version.sh)
architecture=${DEB_ARCHITECTURE:-amd64}
output_root=${1:-target/packages/linux}
package_root="${output_root}/deb-root"
portable_root="${output_root}/InstPlot-Studio-${version}-linux-x86_64"

rm -rf "${output_root}"
mkdir -p \
    "${package_root}/DEBIAN" \
    "${package_root}/usr/bin" \
    "${package_root}/usr/share/applications" \
    "${package_root}/usr/share/doc/instplot-studio" \
    "${package_root}/usr/share/icons/hicolor/512x512/apps" \
    "${portable_root}/bin" \
    "${portable_root}/share/applications" \
    "${portable_root}/share/licenses/instplot-studio" \
    "${portable_root}/share/icons/hicolor/512x512/apps"

cargo build --release --locked --package instplot-studio --bin instplot-studio
binary=target/release/instplot-studio
test -x "${binary}"

install -m 0755 "${binary}" "${package_root}/usr/bin/instplot-studio"
install -m 0644 packaging/linux/instplot-studio.desktop \
    "${package_root}/usr/share/applications/instplot-studio.desktop"
install -m 0644 apps/instplot-studio/assets/InstPlotStudio.png \
    "${package_root}/usr/share/icons/hicolor/512x512/apps/instplot-studio.png"
install -m 0644 LICENSE "${package_root}/usr/share/doc/instplot-studio/copyright"

cat > "${package_root}/DEBIAN/control" <<EOF
Package: instplot-studio
Version: ${version}
Section: science
Priority: optional
Architecture: ${architecture}
Maintainer: InstPlot Studio contributors
Depends: libc6, libgcc-s1, libgl1, libx11-6, libxcb1
Description: Publication-ready scientific plotting application
 InstPlot Studio imports scientific data and exports publication figures.
EOF

install -m 0755 "${binary}" "${portable_root}/bin/instplot-studio"
install -m 0644 packaging/linux/instplot-studio.desktop \
    "${portable_root}/share/applications/instplot-studio.desktop"
install -m 0644 apps/instplot-studio/assets/InstPlotStudio.png \
    "${portable_root}/share/icons/hicolor/512x512/apps/instplot-studio.png"
install -m 0644 LICENSE "${portable_root}/share/licenses/instplot-studio/LICENSE"
ldd "${portable_root}/bin/instplot-studio" > "${portable_root}/DYNAMIC_DEPENDENCIES.txt"

deb="${output_root}/InstPlot-Studio-${version}-linux-x86_64.deb"
portable="${output_root}/InstPlot-Studio-${version}-linux-x86_64.tar.gz"
dpkg-deb --root-owner-group --build "${package_root}" "${deb}"
tar -C "${output_root}" -czf "${portable}" "$(basename "${portable_root}")"
rm -rf "${package_root}" "${portable_root}"

test -s "${deb}"
test -s "${portable}"
printf '%s\n' "${deb}" "${portable}"
