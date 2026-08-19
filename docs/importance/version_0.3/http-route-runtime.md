# v0.3 HTTP Route Runtime Requirements

Semua item pada dokumen ini adalah **Required** sebelum route/controller
metadata digunakan untuk menerima HTTP request.

## Fail-closed application-wide route validation

Bootstrap HTTP harus menjalankan validasi terhadap seluruh controller sebelum
router dibuat. Validasi per-controller v0.1 tidak cukup untuk dua controller
berbeda yang mendeklarasikan method dan pattern yang sama.

**Acceptance criteria:** aplikasi gagal start dengan `MADS030`, menunjukkan
kedua lokasi route, dan tidak membuka listener HTTP.

## Typed handler registration

Nama handler string hanya metadata observability; ia tidak boleh menjadi
mekanisme dispatch. Macro atau adapter harus menghasilkan registrar typed yang
memanggil method trait/controller yang tepat.

**Acceptance criteria:** adapter dapat membangun route tanpa string lookup,
dan route dari dua trait dengan nama method yang sama tetap tidak ambigu.

## Conditional compilation consistency

Metadata, registrar, dan trait method harus memiliki kondisi `cfg` yang sama.
Route yang tidak ada pada build aktif tidak boleh tampak di catalog atau
router.

**Acceptance criteria:** test matrix feature membuktikan route conditional
muncul dan hilang bersama handler-nya.

## Untrusted metadata defense

Adapter memvalidasi prefix, path, canonical full path, HTTP method, controller
identity, dan duplicate descriptor sebelum registrasi. Metadata yang dibuat
manual atau oleh integration crate harus diperlakukan sebagai input yang perlu
divalidasi, bukan sebagai trusted macro output.

## HTTP semantics and observability

v0.3 perlu menetapkan kebijakan eksplisit untuk HEAD/OPTIONS, static versus
dynamic route precedence, trailing slash, dan source-location diagnostic.
Kebijakan tersebut harus diuji pada adapter yang dipilih, bukan hanya pada
metadata foundation.
