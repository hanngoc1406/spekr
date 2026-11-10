<p align="center">
  <img src="assets/icon.png" width="160" height="160" alt="Spekr Logo" />
</p>

<h1 align="center">Spekr</h1>

<p align="center">
  <strong>Phần mềm phân tích phổ âm thanh tốc độ cao, hiện đại được phát triển bằng Rust.</strong><br>
  Công cụ độc lập đa nền tảng cho macOS, Linux và Windows.
</p>

<p align="center">
  <a href="https://github.com/hanngoc1406/spekr/actions/workflows/ci.yml"><img src="https://github.com/hanngoc1406/spekr/actions/workflows/ci.yml/badge.svg?branch=master" alt="Tích hợp liên tục"></a>
  <a href="https://github.com/hanngoc1406/spekr/releases"><img src="https://img.shields.io/badge/phiên_bản-0.1.0-blue.svg" alt="Phiên bản 0.1.0"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/giấy_phép-GPL--3.0-blue.svg" alt="Giấy phép: GPL-3.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/ngôn_ngữ-Rust_1.85%2B-orange.svg" alt="Rust 1.85+"></a>
</p>

<p align="center">
  <a href="#tổng-quan">Tổng quan</a> &bull;
  <a href="#tính-năng-chính">Tính năng</a> &bull;
  <a href="#nền-tảng-công-nghệ">Công nghệ</a> &bull;
  <a href="#cài-đặt">Cài đặt</a> &bull;
  <a href="#hướng-dẫn-sử-dụng">Sử dụng</a> &bull;
  <a href="#kiến-trúc-hệ-thống">Kiến trúc</a> &bull;
  <a href="#giấy-phép-và-tuyên-bố-pháp-lý">Giấy phép</a> &bull;
  <a href="README.md">English Version</a>
</p>

---

## Tổng quan

Spekr là phần mềm mã nguồn mở phân tích phổ âm thanh (acoustic spectrum analyser), được thiết kế phục vụ kỹ sư âm thanh, nhà sản xuất âm nhạc, chuyên gia lưu trữ số và người yêu âm thanh (audiophile) nhằm kiểm tra trực quan chất lượng tín hiệu và tính toàn vẹn của tệp tin âm thanh.

Lấy cảm hứng từ công cụ kinh điển **Spek** của tác giả Alexander Kojevnikov, Spekr là phần mềm phân tích phổ âm thanh hiệu năng cao được xây dựng hoàn toàn bằng ngôn ngữ Rust an toàn bộ nhớ. Ứng dụng mang đến tốc độ biến đổi Fourier nhanh (FFT) đa luồng, giao diện đồ họa tăng tốc phần cứng sắc nét trên màn hình Retina, trình phát âm thanh tích hợp chuyển đổi kiểm âm A/B tức thời và chế độ dòng lệnh tự động hóa toàn diện.

## Tính năng chính

- Hỗ trợ định dạng âm thanh toàn diện: Giải mã trực tiếp các định dạng MP3, FLAC, WAV, AAC, M4A, ALAC, OGG Vorbis, Opus, AIFF và PCM thông qua các bộ giải mã thuần Rust mà không cần cài đặt thư viện FFmpeg ngoài.
- Biến đổi Fourier đa luồng hiệu năng cao: Xử lý dựa trên RustFFT và Rayon, tận dụng tối đa tập lệnh SIMD (AVX2, FMA, NEON) trên toàn bộ các nhân xử lý của CPU.
- Vùng hiển thị phổ sóng tương tác:
  - Phóng to và cuộn mượt mà: Sử dụng con lăn chuột để phóng to/thu nhỏ độc lập trục tần số (kHz) và trục thời gian (mm:ss); kéo chuột trái để di chuyển vùng nhìn.
  - Thước đo và thẻ thông tin thời gian thực (Inspector HUD): Đường chữ thập bám theo con trỏ chuột hiển thị chính xác vị trí thời gian, tần số (Hz) và biên độ âm thanh (dB).
  - Đổi bảng màu tức thì: Hỗ trợ chuyển đổi nhanh giữa các bảng màu SoX (mặc định), Cổ điển (Classic), Magma, Viridis, Inferno, Turbo và Đơn sắc (Monochrome) mà không cần tính toán lại ma trận FFT.
- Trình phát âm thanh tích hợp:
  - Thanh tiến trình toàn chiều rộng với khả năng tua chính xác theo khung mẫu.
  - Đồng hồ hiển thị thời gian phát dạng số và tự động trích xuất ảnh bìa album (Album Art) từ metadata.
  - Chế độ so sánh A/B tức thì: Chuyển đổi linh hoạt giữa hai bản nhạc tại cùng một vị trí thời gian để đối chiếu trực tiếp chất lượng giữa bản gốc và bản nén.
- Xử lý hàng loạt và danh sách phát:
  - Hỗ trợ mở thư mục hoặc kéo thả nhiều tệp cùng lúc.
  - Cột danh sách phát (Playlist) bên trái giúp chuyển đổi bản nhạc nhanh chóng.
  - Xuất ảnh phổ tần PNG hàng loạt đa luồng cho toàn bộ album.
- Chuẩn hóa hiển thị văn bản đa ngôn ngữ: Tích hợp sẵn phông chữ Be Vietnam Pro cùng cơ chế tự động chuyển đổi Unicode NFC và phông chữ CJK dự phòng của hệ điều hành, đảm bảo hiển thị chuẩn xác tiếng Việt và các ngôn ngữ Á Đông trên mọi hệ điều hành.
- Chế độ dòng lệnh không cần giao diện (Headless CLI): Hỗ trợ lập trình kịch bản tự động kiểm tra chất lượng tệp tin trên máy chủ hoặc quy trình CI/CD.

## Nền tảng công nghệ

| Thành phần | Công nghệ sử dụng | Đặc tính nổi bật |
| :--- | :--- | :--- |
| Giải mã âm thanh | Symphonia | Bộ giải mã thuần Rust 100%, không phụ thuộc thư viện C ngoài |
| Xử lý FFT | RustFFT + Rayon | Đường ống STFT song song đa luồng tối ưu hóa tập lệnh SIMD |
| Giao diện người dùng | eframe / egui | Tăng tốc đồ họa phần cứng GPU ở 60 FPS, hỗ trợ Retina/HiDPI |
| Động cơ âm thanh | Rodio | Luồng phát âm thanh thời gian thực với khả năng tua chính xác |
| Hệ thống phông chữ | ab_glyph + Be Vietnam Pro | Nhúng sẵn phông chữ và tự động chuẩn hóa Unicode NFC |
| Động cơ CLI | Clap | Tự động hóa kết xuất hình ảnh phục vụ kiểm tra chất lượng |
| Đóng gói nhị phân | Tệp thực thi độc lập | Single static binary hoàn chỉnh, chạy ngay không cần cài thêm thư viện |

## Cài đặt

### Bản biên dịch sẵn (Liên kết tải trực tiếp)

Liên kết tải trực tiếp cho bản phát hành ổn định mới nhất:

| Hệ điều hành | Kiến trúc | Tải về trực tiếp |
| :--- | :--- | :--- |
| **macOS** | Universal (Apple Silicon & Intel) | [**Spekr-macos.dmg**](https://github.com/hanngoc1406/spekr/releases/latest/download/Spekr-macos.dmg) |
| **Windows** | x86_64 (64-bit) | [**Spekr-windows-x86_64.zip**](https://github.com/hanngoc1406/spekr/releases/latest/download/Spekr-windows-x86_64.zip) (Bản nén Portable) |
| **Windows** | x86_64 (64-bit) | [**Spekr-windows-x86_64.exe**](https://github.com/hanngoc1406/spekr/releases/latest/download/Spekr-windows-x86_64.exe) |
| **Linux** | x86_64 (glibc 2.31+) | [**Spekr-linux-x86_64.tar.gz**](https://github.com/hanngoc1406/spekr/releases/latest/download/Spekr-linux-x86_64.tar.gz) |

Người dùng dòng lệnh trên macOS có thể tải các bản nén CLI: [**spekr-macos-arm64.tar.gz**](https://github.com/hanngoc1406/spekr/releases/latest/download/spekr-macos-arm64.tar.gz) (Apple Silicon) và [**spekr-macos-x86_64.tar.gz**](https://github.com/hanngoc1406/spekr/releases/latest/download/spekr-macos-x86_64.tar.gz) (Intel). Toàn bộ danh mục phát hành và lịch sử phiên bản có thể xem tại trang [Releases](https://github.com/hanngoc1406/spekr/releases).

Giải nén tệp lưu trữ và đưa tệp thực thi vào biến môi trường PATH của hệ thống.

### Biên dịch từ mã nguồn

Yêu cầu: Đã cài đặt [Rust toolchain](https://rustup.rs/) (phiên bản 1.85 trở lên).

```bash
# Tải mã nguồn về máy
git clone https://github.com/hanngoc1406/spekr.git
cd spekr

# Biên dịch bản phát hành tối ưu
cargo build --release

# Tệp nhị phân hoàn tất nằm tại:
# target/release/spekr (Unix) hoặc target/release/spekr.exe (Windows)
```

Trên hệ điều hành Linux, cần cài đặt thêm thư viện phát triển âm thanh ALSA trước khi biên dịch:
```bash
# Debian / Ubuntu
sudo apt-get install -y libasound2-dev libgl1-mesa-dev

# Fedora / RHEL
sudo dnf install -y alsa-lib-devel mesa-libGL-devel

# Arch Linux
sudo pacman -S alsa-lib
```

## Hướng dẫn sử dụng

### Chế độ giao diện đồ họa (GUI)

Khởi chạy không tham số để mở cửa sổ ứng dụng, hoặc truyền trực tiếp đường dẫn tệp tin âm thanh hoặc thư mục:

```bash
# Khởi chạy ứng dụng
spekr

# Mở trực tiếp một tệp âm thanh
spekr recording.flac

# Mở toàn bộ thư mục album
spekr /path/to/album/
```

#### Phím tắt thao tác

- `Phím cách (Space)`: Phát hoặc tạm dừng âm thanh.
- `Con lăn chuột`: Phóng to hoặc thu nhỏ vùng phổ tại vị trí con trỏ.
- `Nhấp giữ chuột trái + Kéo`: Di chuyển qua lại vùng phổ khi đang phóng to.
- `Nhấp đúp chuột trái`: Đặt lại mức thu phóng về toàn cảnh mặc định.

### Chế độ dòng lệnh (Headless CLI)

Spekr hỗ trợ xuất ảnh phổ tần tự động thông qua dòng lệnh mà không cần khởi động giao diện đồ họa:

```bash
# Xuất ảnh phổ tần mặc định
spekr input.flac -o spectrogram.png

# Xuất ảnh độ phân giải cao với bảng màu và hàm cửa sổ tùy chọn
spekr input.flac -o output.png --width 1920 --height 1080 --fft 4096 --palette viridis --window blackman-harris

# Kiểm tra kênh âm thanh trái với khoảng dải động tùy biến
spekr track.wav -o left_channel.png --channel 1 --palette sox --min-db -100.0 --max-db 0.0

# Xuất ảnh hàng loạt cho toàn bộ thư mục âm thanh
spekr /path/to/audio/folder -o /path/to/output/folder
```

#### Danh mục tham số dòng lệnh

```text
Usage: spekr [OPTIONS] [AUDIO_FILE]

Arguments:
  [AUDIO_FILE]  Đường dẫn tệp âm thanh đầu vào hoặc thư mục

Options:
  -o, --output <PATH>      Đường dẫn tệp ảnh PNG xuất ra hoặc thư mục đích
      --width <INT>        Chiều rộng ảnh tính bằng pixel [mặc định: 1200]
      --height <INT>       Chiều cao ảnh tính bằng pixel [mặc định: 600]
      --fft <INT>          Kích thước cửa sổ FFT: 1024, 2048, 4096, 8192 [mặc định: 2048]
      --window <WINDOW>    Hàm cửa sổ: hann, hamming, blackman-harris, flattop, rect [mặc định: hann]
      --palette <PALETTE>  Bảng màu: sox, classic, magma, viridis, inferno, turbo, mono [mặc định: sox]
      --min-db <FLOAT>     Ngưỡng biên độ dưới tính bằng decibel [mặc định: -120.0]
      --max-db <FLOAT>     Ngưỡng biên độ trên tính bằng decibel [mặc định: 0.0]
      --channel <CHANNEL>  Lựa chọn kênh âm thanh: mix, 1 (trái), 2 (phải) [mặc định: mix]
  -h, --help               Hiển thị danh mục hướng dẫn
  -V, --version            Hiển thị thông tin phiên bản
```

## Kiến trúc hệ thống

Mã nguồn dự án được tổ chức theo các mô-đun độc lập:

- `src/audio.rs`: Giải mã luồng âm thanh, phân tích siêu dữ liệu và trích xuất ảnh bìa album bằng `symphonia`.
- `src/fft.rs`: Cấu hình hàm cửa sổ và tính toán biến đổi Fourier nhanh bằng `rustfft`.
- `src/pipeline.rs`: Đường ống xử lý song song các cột phổ STFT đa luồng bằng `rayon`.
- `src/render.rs`: Kết xuất ảnh phổ tĩnh, chống răng cưa cho thước đo tọa độ, nhãn thông số và thang màu.
- `src/playback.rs`: Quản lý luồng âm thanh và điều khiển phát nhạc thời gian thực bằng `rodio`.
- `src/palette.rs`: Xử lý không gian màu và ánh xạ các thang màu đồng nhất cảm thụ thị giác.
- `src/fonts.rs`: Hệ thống phông chữ nhúng TrueType và chuẩn hóa Unicode NFC tự động.
- `src/batch.rs`: Quét cấu trúc thư mục và điều phối tiến trình kết xuất hàng loạt.
- `src/app.rs`: Quản lý máy trạng thái và kết xuất giao diện người dùng bằng `eframe` và `egui`.
- `src/cli.rs`: Định nghĩa cấu trúc tham số và xử lý dòng lệnh bằng `clap`.

## Đóng góp phát triển

Mọi đóng góp từ cộng đồng đều được hoan nghênh theo quy trình tiêu chuẩn:

1. Tạo bản sao (Fork) kho lưu trữ và tạo nhánh tính năng (`git checkout -b feature/improvement`).
2. Kiểm tra chất lượng mã nguồn và vượt qua toàn bộ các bài kiểm thử tự động:
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```
3. Lưu vết thay đổi (`git commit -m 'Implement feature'`).
4. Đẩy mã nguồn lên nhánh cá nhân (`git push origin feature/improvement`) và tạo Yêu cầu hợp nhất (Pull Request).

## Giấy phép và Tuyên bố pháp lý

### Giấy phép

Dự án được phân phối dưới giấy phép [GNU General Public License v3.0 (GPL-3.0)](LICENSE).

### Ghi nhận công lao và Tuyên bố độc lập

Spekr là một dự án mã nguồn mở độc lập được phát triển bằng ngôn ngữ Rust. Dự án lấy cảm hứng từ phần mềm **Spek** kinh điển do tác giả **Alexander Kojevnikov** ([spek.cc](http://spek.cc)) sáng lập.

Spekr hoàn toàn độc lập, không liên kết, không đại diện và không được bảo trợ bởi dự án Spek nguyên bản hoặc tác giả gốc. Tất cả các nhãn hiệu và tên thương mại thuộc quyền sở hữu của các chủ thể tương ứng.
