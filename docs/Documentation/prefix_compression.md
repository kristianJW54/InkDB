## Prefix Compression

For pages, two forms of compresssion come to mind: Prefix and Suffix compression.

Prefix compression takes a common prefix for all keys in a page and stores only the suffixes of the key to save space.

**Example**
| Common Prefix | Suffix | Prefix Len | Suffix Len | Total Space Saved |
|---------------|--------|------------|------------|-------------------|
| "user_"       | "123"  | 5          | 3          | 5 bytes
| "user_"       | "456"  | 5          | 3          | 5 bytes
| "user_"       | "789"  | 5          | 3          | 5 bytes
|                |       |             |           |  **15 bytes**


The problem is how to handle this on a page level. A page is a raw collection of bytes split by:
- Header
- Slot Array
- Free Space
  - Where free space houses cell data
  - Each cell holds a key and value/child pointer
