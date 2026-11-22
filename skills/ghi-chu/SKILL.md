---
name: ghi-chu
description: Quy ước làm việc với Studio Ghi chú, gồm thứ tự ưu tiên của ghi chú và khi nào nên đề xuất thay vì sửa thẳng. Dùng khi người dùng hỏi nên làm việc nào trước, hoặc nhờ thêm, sửa, sắp xếp ghi chú.
---
# Studio Ghi chú

## Thứ tự ưu tiên

- Ghi chú có tiêu đề mở đầu bằng `!` là việc gấp. Làm việc gấp trước mọi ghi chú khác.
- Nếu có nhiều việc gấp, hoặc không có việc gấp nào, thì làm theo thứ tự trong danh sách, từ trên xuống.
- Khi trả lời nên làm việc nào trước, nêu đúng tiêu đề ghi chú, kể cả dấu `!`.

## Thêm và sửa

- Khi người dùng nhờ đổi tên một ghi chú do chính họ tạo, hãy đề xuất bằng `notes.propose_rename` để họ xem rồi chấp nhận trong Studio.
- Chỉ thêm thẳng bằng `notes.add` khi người dùng nói rõ là muốn thêm.
- Muốn đánh dấu một việc là gấp thì thêm `!` vào đầu tiêu đề. Không dùng ký hiệu nào khác.
