# v0.2 Follow-up Findings

**Status:** Required for migration clarity

Temuan tambahan dari review proposed v0.2 tidak mengubah desain graph atau
kontrak API utama. Task 1 sampai Task 9 dapat dimigrasikan terlebih dahulu
secara berurutan, kemudian follow-up berikut dapat ditambahkan:

- dokumentasi eksplisit bahwa dependency concrete pada field harus `Clone`;
- penjelasan bahwa managed provider dengan handle bersama merupakan pilihan
  standar untuk resource application-scoped;
- penamaan eksplisit bahwa `Inject<T>` dan automatic trait injection belum
  termasuk scope v0.2;
- integration test yang memastikan self-cycle atau multi-node cycle
  menghasilkan `MADS005` dan menjalankan zero constructor;
- catatan bahwa optimasi allocation catalog tetap deferred sampai benchmark
  bootstrap menunjukkan biaya yang relevan.

Satu koreksi harus konsisten saat Task 4 dimigrasikan dan tidak boleh ditunda
sebagai fitur baru: dua provider berbeda yang menghasilkan concrete type yang
sama harus menghasilkan `MADS002` (ambiguous provider binding), sedangkan
`MADS001` hanya untuk exact duplicate descriptor identity. Snapshot Task 6 dan
setelahnya harus mempertahankan aturan ini.

Direktori `docs/proposed/task_n` diperlakukan sebagai perubahan berurutan,
bukan snapshot repository lengkap. Setiap `MANIFEST.md` harus dibaca bersama
hasil migrasi task sebelumnya; perubahan pada task berikutnya tidak boleh
menghapus file atau kontrak dari task sebelumnya.
