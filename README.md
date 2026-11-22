# Ghi chú — Studio mẫu cho LynaX

Studio mẫu của LynaX có lõi native viết bằng Rust: ghi chú, hoàn tác, và đề xuất của agent chỉ áp dụng khi bạn chấp nhận.

## Cài vào LynaX Desktop

Studio › **Từ GitHub** › nhập `vocongdanh/lynax-studio-ghi-chu`, để trống ô thẻ để lấy bản phát hành mới nhất.

Lõi native chỉ cài được từ **bản phát hành** (tệp `.lxstudio` đã dựng sẵn); LynaX không dựng mã Rust từ repo.

## Cấu trúc

- `studio.json`: manifest của Studio.
- `ui/`: giao diện; `migrations/`: lược đồ dữ liệu; `skills/`: hướng dẫn cho agent.
- `core-src/`: mã nguồn lõi (`cargo build --release`), đóng gói thành `core/<hệ điều hành>-<kiến trúc>/notes-core` trong bản phát hành.
