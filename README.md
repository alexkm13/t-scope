# T-Scope

T-Scope is a live MBTA operations display built around a cross-process shared-memory protocol.

A collector process pulls current MBTA vehicle and prediction data, normalizes it into one whole-system snapshot, and publishes that snapshot through an `mmap`-backed shared region. A separate display process reads the latest coherent snapshot without calling the MBTA API itself.

The project is primarily a systems exercise in:

- cross-process shared memory
- bounded snapshot publication
- safe memory reuse
- atomic publication
- reader/writer coordination
- fixed-layout shared representations

## Pipeline

```text
MBTA /vehicles + /predictions
            ↓
      collector process
            ↓
       domain Snapshot
            ↓
   fixed shared representation
            ↓
       mmap SharedRegion
            ↓
  3-slot publication protocol
            ↓
       display process
```

## Shared-memory design

T-Scope publishes one logical snapshot of the whole system at a time.

The shared region contains:

```text
SharedRegion
├── reader_holds
├── publication
├── slot 0: SharedSnapshot
├── slot 1: SharedSnapshot
└── slot 2: SharedSnapshot
```

`publication` is a packed atomic value containing:

```text
high 32 bits → generation
low 32 bits  → published slot
```

`reader_holds` records which slot the display process is currently reading.

The three slots have three possible roles:

```text
old snapshot still held by reader
current published snapshot
writer scratch slot for N+1
```

The writer never overwrites either the published slot or the reader-held slot.

## Reader protocol

The reader cannot simply load the published slot and immediately traverse it, because the writer could publish a newer snapshot and recycle the old slot before the reader announces that it is using it.

The display therefore uses a claim-and-verify protocol:

```text
load publication
→ decode published slot
→ claim slot in reader_holds
→ load publication again
→ if unchanged, read snapshot
→ otherwise release and retry
→ release reader_holds after reading
```

This prevents the reader from traversing memory that the writer may already be reusing.

## Writer protocol

The collector reads:

```text
published slot
reader-held slot
```

and selects one of the remaining safe slots.

It then:

```text
builds N+1 completely in the selected slot
→ increments generation
→ publishes {generation, slot} with a Release store
```

The payload is written before publication, so an incomplete snapshot never replaces the last completed one.

## Shared representation

The process-local model uses ergonomic Rust types such as:

```text
String
Vec<T>
Option<T>
DateTime
```

These cannot be copied directly into shared memory because they contain process-local representation such as heap pointers.

T-Scope instead uses fixed-layout `#[repr(C)]` shared structs built from:

```text
fixed-size arrays
integers
floats
explicit counts
explicit missing-value sentinels
```

Examples:

```text
Vec<TrainState>
→ [SharedTrainState; MAX_TRAINS] + train_count

Vec<Prediction>
→ [SharedPrediction; MAX_PREDICTIONS] + prediction_count

String
→ fixed [u8; N] representation
```

Capacity overflow is treated as an error rather than silently truncating data.

## Direct mmap writes

`SharedSnapshot` is intentionally large, so constructing a full `SharedRegion` or `SharedSnapshot` as a temporary stack value can overflow the stack.

The final writer avoids this by writing the encoded snapshot directly into the selected mmap-backed slot rather than constructing the entire region on the stack and copying it afterward.

## MBTA data model

The collector currently combines:

- `/vehicles`
- `/predictions`

Vehicle state and prediction rows are joined by stable `vehicle_id`.

A vehicle can have multiple current prediction records, so predictions are stored as a collection per train rather than using last-write-wins semantics.

## Behavior

The collector continuously:

```text
fetches MBTA state
→ builds one normalized Snapshot
→ chooses a safe shared-memory slot
→ writes directly into that slot
→ publishes it
→ repeats
```

The display continuously reads the latest coherent publication.

Intermediate snapshots may be skipped. T-Scope is a latest-state system, not an event log.

## Correctness properties

The protocol is designed so that:

- the reader never reads a slot currently being overwritten
- the writer never overwrites the currently published snapshot
- the writer never overwrites a reader-held snapshot
- a partially written N+1 is never published
- a slow reader does not permanently block the writer
- memory usage is bounded
- the reader can start before the first valid publication
- publication identity is observed atomically

## Running

Run the collector:

```bash
cargo run --bin collector
```

Run the display in another terminal:

```bash
cargo run --bin display
```

Both processes use the same `shared_data.dat` backing file.

## Validation

The implementation was tested against the important protocol cases:

- normal collector/display operation
- repeated publication across the three slots
- reader starting before the first publication
- slow reader behavior
- rapid writer updates
- writer interruption / restart behavior

The goal of these tests is not merely to show that data appears on screen, but to attack slot reuse and publication invariants.

## What I learned

T-Scope was primarily a shared-memory and reclamation project.

The most reusable lessons were:

- `mmap` maps the same backing bytes into separate process address spaces
- `String` and `Vec` are not valid cross-process shared representations
- fixed-layout representations make shared memory explicit
- `&mut T` means exclusive Rust access, not merely “writable”
- raw pointers are sometimes necessary when external shared memory cannot satisfy Rust's normal aliasing model
- atomic publication can combine multiple logical fields through bit packing
- safe bounded-memory reuse requires proving when memory is no longer in use
- version validation alone is not enough if ordinary payload memory can still be concurrently overwritten
- simple-looking writer logic depends on a correct reader/reclamation protocol

## Postmortem

The hardest part of T-Scope was not the MBTA API or the display layer. It was building a shared-memory protocol whose ownership model remained correct across process boundaries.

The first implementation attempts mixed several concerns at once: API ingestion, mmap mechanics, fixed-layout encoding, unsafe Rust, and slot reclamation. The project became much cleaner once those were separated into:

```text
domain model
shared representation
shared-memory layout
reader protocol
writer protocol
```

A second implementation lesson was avoiding giant stack temporaries. `SharedRegion` is large enough that treating it like an ordinary local Rust value is the wrong model; it should be viewed as storage that already exists inside the mmap and is filled in place.

The final design is intentionally small:

```text
one writer
one reader
three bounded slots
one atomic publication identity
one reader-held slot indicator
```

That was enough to make the cross-process snapshot publication safe without turning the project into a general-purpose messaging system.
