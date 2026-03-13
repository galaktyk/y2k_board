start implement sprint 2

# Sprint 2
* ops CRDT system
* snapshot save/load

```
snapshot.bin
```

Contains:

* full board state
* no history
* deleted elements removed


# Ops

Board changes are represented as **operations**.

Ops follow a CRDT-friendly model for future collaboration.

Operation types:

```
ADD_ELEMENT
DELETE_ELEMENT
SET_PROPERTY
... more to come
```

only fire when finalized (e.g. on mouse up) to avoid spamming updates during dragging.
for debug function like alt+B, don't fire ops

also the input.rs is too large now, please separte file and folder