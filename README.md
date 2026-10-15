# Snip — Snipping tool giống Win+Shift+S cho Ubuntu

Viết bằng Rust + GTK3, chạy trên GNOME (Wayland và X11).

Yêu cầu: Ubuntu 22.04 trở lên, desktop GNOME (mặc định của Ubuntu).

## Cài đặt

```bash
wget https://github.com/TuanNH92/snip/releases/latest/download/snip_amd64.deb
sudo apt install ./snip_amd64.deb
```

Sau đó mở **Snip** từ menu ứng dụng một lần — phím tắt **Super+Shift+S** sẽ được gán tự động.
Đổi phím tắt trong *Settings → Keyboard → Keyboard Shortcuts → Custom Shortcuts*.

Gỡ cài đặt: `sudo apt remove snip`

### Build từ mã nguồn

```bash
sudo apt install build-essential libgtk-3-dev   # + Rust: https://rustup.rs
./install.sh      # build, cài vào ~/.local/bin/snip, gán phím Super+Shift+S
./uninstall.sh    # gỡ
```

Đổi phím tắt: `SNIP_SHORTCUT='<Ctrl><Alt>s' ./install.sh`

### Phát hành phiên bản mới (cho maintainer)

```bash
# sửa version trong Cargo.toml, rồi:
git tag v0.1.0 && git push origin v0.1.0
```

GitHub Actions (`.github/workflows/release.yml`) tự build `.deb` trên Ubuntu 22.04 và đính kèm vào Release.

## Sử dụng

Nhấn **Super+Shift+S** (hoặc chạy `snip`) → màn hình đóng băng và tối lại:

| Chế độ          | Thao tác                                           |
|-----------------|----------------------------------------------------|
| ▭ Chữ nhật      | kéo chuột để chọn vùng                             |
| ✎ Tự do         | vẽ vùng bất kỳ (phần ngoài vùng sẽ trong suốt)     |
| ⛶ Toàn màn hình | click vào màn hình cần chụp                        |
| Hủy             | `Esc`, chuột phải, hoặc nút ✕                      |

Sau khi chụp, ảnh được:
- **sao chép vào clipboard** (Ctrl+V để dán),
- lưu vào `~/Pictures/Screenshots/Snip_<thời gian>.png`,
- kèm thông báo — bấm vào để mở trình chỉnh sửa (bút, dạ quang, hoàn tác `Ctrl+Z`,
  sao chép `Ctrl+C`, lưu `Ctrl+S`).

## Ghi chú kỹ thuật

- **Chụp màn hình**: Wayland không cho app đọc màn hình. Snip tạm giữ tên D-Bus
  `org.gnome.Screenshot` (nằm trong allowlist của GNOME Shell) để gọi
  `org.gnome.Shell.Screenshot` — không flash, không hộp thoại xin quyền. Dự phòng: `gnome-screenshot`.
- **Chạy nền**: trên Wayland clipboard mất khi app thoát, nên `snip` là tiến trình
  single-instance chạy nền giữ clipboard; lần nhấn phím tiếp theo mở overlay ngay.
  Tắt hẳn: `gapplication action io.github.tuannh92.Snip quit`.
- **Nhiều màn hình**: mỗi monitor có một overlay riêng (Wayland không cho một cửa sổ trải
  qua nhiều màn hình), nên một vùng chọn nằm trong một màn hình.
- Không có chế độ "chụp cửa sổ" như Windows vì Wayland không cho app liệt kê cửa sổ khác.
