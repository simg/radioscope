PKG_CONFIG=/usr/bin/pkg-config \
PKG_CONFIG_ALLOW_CROSS=1 \
PKG_CONFIG_SYSROOT_DIR=/opt/pi-sysroot \
PKG_CONFIG_LIBDIR=/opt/pi-sysroot/usr/lib/aarch64-linux-gnu/pkgconfig \
PKG_CONFIG_PATH=/opt/pi-sysroot/usr/lib/aarch64-linux-gnu/pkgconfig \
cargo build --release --target aarch64-unknown-linux-gnu