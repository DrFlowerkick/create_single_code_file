# Create single rust code file for codingame and similar challenges

[codingame](www.codingame.de) requires a single file containing all required source code. It also provides only a small selection of crates from [crates.io](crates.io). Therefore if you want to use reusable code from a library, you have to

1. manage your own library locally on your system and
2. you have to merge manually your challenge source code with your local library code into one file.

Since this is an annoying and boring task I wrote the tool **create_codingame_single_file** to do the manual work for me.

## Content

This repository contains three rust projects:

1. **csf_cg**: The project which contains the code for the merge tool **create_codingame_single_file**.
2. **csf_cg_binary_test**: A simple rust project which depends on the library **csf_cg_lib_test**.
3. **csf_cg_lib_test**: A small library with a collection of modules, which are used by **csf_cg_binary_test**.

**csf_cg_binary_test** and **csf_cg_lib_test** are used for testing of **csf_cg**.
