**Dev Log**

20/01/2026 - 22:57 GMT

## 001-Pages

This is the first dev log for the InkDB project. What is the purpose of this log? It is to mainly convey the process of thinking and outlining of logic behind some of the concepts in the project. It is an audit trail of sorts that I can come back to and learn from and see where certain mistakes were made or certain design decisions were developed.

Being that I have decided to do this log a little further into development than starting from scratch, I will summarise what I have done so far and go into what I have done as of now.

So far, I have implemented a Slotted Page layout for database pages to use. This layout is effectively the base structure for all pages stored on disk and in memory. The layout is defined as follows:


| Header | Slot Array | Slot Data | Special Space |
|--------|------------|-----------|---------------|
| 24 bytes | 4 bytes * N | Variable length | 8 bytes Sibling Pointers |  

For code structure, I decided to use a Ref and Mut Slotted Page struct as a wrapper around a RawPage type of **[u8; 4096]**  

```rust
pub(crate) struct SlottedPageMut<'a> {
    bytes: &'a mut RawPage,
}
//
pub(crate) struct SlottedPageRef<'a> {
    bytes: &'a RawPage,
}
```

The mechanics of a slotted page are well documented. Essentially, the header contains the metadata for the page, including helpful informations such a free_start, free_end, page_type and flags etc. I followed the header layout similar to PostgreSQL's page layout. Much of the slotted page workings and methods will be found in the Documentation folder where I go into detail about the architecture of the slotted page and design InkDB implements.

For the sake of the dev log i will keep this high level and not go into too much detail about things i have already implemented.

It is worth mentioning the contextual layers above the Slotted Page and how they knit together in order to highlight the current progress made in this dev log iteration.

A Slotted Page can only be given out by a FrameGaurd. The FrameGuard holds an ``` RwLockReadGuard<'_> ``` or an ``` RwLockWriteGuard<'_> ``` These are the latches which are distributed by a **PageFrame** (more on this in the Documentation)

So once we have and hold a Slotted Page, depending on the latch type we know that we can either read or write and are protected by the lifetime of the FrameGuard.

// TODO: Explain the page speicifc layers and what we have been working on

// TODO: Talk about deciding the layout, flags, special space header etc

// TODOD: Prefix compression
