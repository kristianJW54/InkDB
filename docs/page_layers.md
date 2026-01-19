// TODO: Document the page layer architecture and it's safety guarantees.


## Page Structures and Layers

At the core of InkDB is the Slotted Page. This is the standard page layout and engine that all pages are built on and use to store and manage data. Different types of pages use the Slotted Page structure to store and manage it's data by calling into the Slotted Page's low level API to manipulate the bytes and layout of the page.

As with most traditional databases the Slotted Page consists of:
- A header
- Slot Array
- Free Space for Cell Data
- Special Space for Sibling Pointers

// NOTE: Use graph to visualize the page structure and layers
