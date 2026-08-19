# v0.2 Dependency Ownership and Graph Decisions

## Dependency field cloning

**Status:** Required

Managed service, repository, dan controller saat ini memperoleh dependency
dengan `resolve::<T>()?.as_ref().clone()`. Untuk managed handle, clone hanya
menambah reference count pada allocation bersama. Untuk concrete value biasa,
clone dapat mahal atau bahkan tidak tersedia.

v0.2 graph dan documentation harus mendefinisikan kontrak ini secara eksplisit:

- field dependency concrete harus `Clone`;
- managed provider adalah pilihan standar untuk resource bersama;
- `Inject<T>` atau binding trait tidak termasuk scope v0.2 kecuali spec graph
  diperluas secara eksplisit.

**Acceptance criteria:** graph diagnostic dapat menunjukkan dependency yang
hilang atau cyclic sebelum constructor berjalan, dan dokumentasi tidak
menjanjikan automatic trait injection.

## Self and cyclic dependencies

**Status:** Required

Macro dapat mendeskripsikan dependency pada type yang sama melalui `Self`.
v0.2 harus mendeteksi self-cycle dan multi-node cycle sebagai graph error
deterministik (`MADS005`) sebelum allocation provider dilakukan.

## Catalog allocation optimization

**Status:** Deferred; measure first

Catalog provider sudah memakai cache startup. Route catalog masih meng-clone
collection untuk API publik. Optimasi index `TypeId` atau borrowed iterator
hanya dilakukan bila benchmark bootstrap menunjukkan biaya relevan.
