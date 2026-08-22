# Important Implementation Decisions

Folder ini mencatat temuan arsitektur yang belum seluruhnya menjadi pekerjaan
milestone saat ini. Setiap temuan ditempatkan pada versi pertama yang memiliki
scope dan boundary tepat untuk menyelesaikannya.

| Milestone | Dokumen | Fokus |
| --- | --- | --- |
| v0.1 | [Route/controller foundation](version_0.1/route-controller-foundation.md) | Contract metadata dan validasi statis tanpa HTTP runtime. |
| v0.2 | [Dependency ownership](version_0.2/dependency-ownership.md), [follow-up findings](version_0.2/follow_up.md) | Graph provider, ownership dependency, construction planning, dan temuan migrasi lanjutan. |
| v0.3 | [HTTP route runtime](version_0.3/http-route-runtime.md) | Axum adapter, dispatch handler, dan validasi route application-wide. |
| v0.4 | [Diesel persistence](version_0.4/diesel-persistence.md) | Explicit PostgreSQL/Diesel pool, migrations, and release gates. |

Status dalam dokumen:

- **Implemented**: sudah menjadi perilaku yang diuji pada milestone tersebut.
- **Required**: harus selesai sebelum milestone dapat dianggap complete.
- **Deferred**: sengaja tidak dikerjakan sekarang karena membutuhkan boundary
  milestone lain; bukan backlog tanpa owner.
