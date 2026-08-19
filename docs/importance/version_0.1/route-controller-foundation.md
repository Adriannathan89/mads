# v0.1 Route and Controller Foundation

## Boundary

v0.1 hanya menyediakan contract trait, metadata route statis, managed
controller, dan validasi foundation. Tidak ada Axum router, extractor,
response conversion, atau HTTP request dispatch pada versi ini.

## Implemented hardening

### Equivalent dynamic route patterns conflict

**Status:** Implemented

Conflict detection memperlakukan seluruh parameter path sebagai wildcard
struktural. Karena itu `GET /users/:id` dan `GET /users/:user_id` tidak dapat
terdaftar pada controller yang sama. Nama parameter tetap dipertahankan pada
metadata untuk dokumentasi dan adapter masa depan, tetapi bukan bagian dari
identity konflik.

**Acceptance criteria:** construction controller gagal dengan `MADS030` bila
dua route memiliki HTTP method dan pola path yang ekuivalen.

### Control characters are rejected in route paths

**Status:** Implemented

Prefix dan endpoint path menolak semua Unicode control character, selain
validasi yang sudah ada untuk query, fragment, percent-encoding, whitespace,
dan backslash.

**Acceptance criteria:** `#[get("/health\\0check")]` gagal saat compile dengan
diagnostic yang jelas.

## Deferred decisions

### Endpoint attribute identity

**Status:** Deferred to v0.3 design review

Saat ini macro mengenali endpoint attribute berdasarkan segmen nama terakhir,
misalnya `#[mads::get(...)]`. Penyempitan ke namespace MADS yang eksplisit
berpotensi menjadi breaking change untuk facade atau dependency yang di-rename.
Sebelum v0.3, desain harus menetapkan bentuk attribute yang didukung dan
menambahkan compile test untuk dependency rename.

### Conditional route declarations

**Status:** Deferred to v0.3

Route yang menggunakan `#[cfg]` atau `#[cfg_attr]` harus menghasilkan metadata
yang identik dengan method yang benar-benar tersedia pada build aktif. Ini
penting ketika metadata menjadi input router, tetapi tidak diperlukan untuk
v0.1 yang belum mengeksekusi HTTP.

### External metadata constructors

**Status:** Deferred to v0.3

`RouteDescriptor::new` dan `ControllerRouteDescriptor::new` harus tetap public
karena proc macro diekspansi di crate pengguna. Runtime adapter v0.3 wajib
memvalidasi metadata yang diterimanya dan menolak descriptor yang tidak
konsisten, termasuk duplicate controller identity.

### Allocation model

**Status:** Deferred; benchmark before redesign

Managed controller memakai satu `Arc` untuk handle dan satu `Arc` untuk
type-erased registry storage. Ini bukan memory leak dan hanya terjadi saat
startup. Tidak ada perubahan ownership sebelum benchmark menunjukkan biaya
yang berarti pada aplikasi nyata.
