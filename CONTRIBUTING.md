# Contributing to archival

If you're new to open source, the [Open Source Guides](https://opensource.guide/) is a good place to learn about how things usually work. Here are some especially helpful guides for folks just getting started:

- [How to Contribute to Open Source](https://opensource.guide/how-to-contribute/)
- [Building Welcoming Communities](https://opensource.guide/building-community/)

## Helping make archival better

archival is intentionally a very small tool, but since it runs in lots of different environments (OSs, WASM, CI), and has an ecosystem of sites and hosts, there's plenty of ways it can be improved. Here are some of the ones we focus on.

- Build things with it - archival makes different choices about data, which means there are always new and interesting ways to use it. If you find your use case isn't smooth, reach out or [file an issue](#reporting-new-issues).
- Check out the [open issues](https://github.com/jesseditson/archival/issues) to see if anything looks fun to fix. We use the tag [good first issue](https://github.com/jesseditson/archival/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22) to indicate ones that are good for folks who haven't contributed yet.
- Join the [discord](https://archival.dev/chat.html) and ask questions, get the latest updates, and chat with us about your projects.

The editor and documentation sites are also open source, and while the editor is our product, it's also an example of how to build complex things with archival. If you find issues in either, please file an issue or a PR!

- https://github.com/jesseditson/archival-docs
- https://github.com/jesseditson/archival-editor

### Reporting new issues

The public issue tracker is [here on github](https://github.com/jesseditson/archival/issues) - when reporting an issue, please do your best to put it in the right repo - for instance, if you're experiencing issues with the editor, file it in the [editor repo](https://github.com/jesseditson/archival-editor/issues). For template issues, use the [template repo](https://github.com/jesseditson/archival-website/issues). For documentation issues, use the [documentation repo](https://github.com/jesseditson/archival-docs/issues).

### Installation

To work on the archival library, all you need is the rust toolchain:

https://doc.rust-lang.org/cargo/getting-started/installation.html

Then, you can clone this repo and develop it using `cargo`.

### Developing

While we're still young, just ask for help on [discord](https://archival.dev/chat.html) - at a high level, the main thing to be aware of is that archival is cross-compiled into binaries for every major OS, mobile OSes, and WASM - so it's important to run the tests and pay attention to which features your code will run under.

To assist in avoiding finding about issues before breaking in CI, add a pre-commit hook:

```bash
echo "./pre-commit.sh" > .git/hooks/pre-commit
```

### Creating a branch

Fork [the repository](https://github.com/jesseditson/archival) and create your branch from `main`. If you've never sent a GitHub pull request before, you can learn how from [this free video series](https://egghead.io/courses/how-to-contribute-to-an-open-source-project-on-github).

### Testing

archival uses a vanilla cargo setup for tests, and all tests can be run with `./test.sh` in the root of the repo.

### Style guide

archival uses `clippy` with default settings as the linter. See docs:
https://doc.rust-lang.org/stable/clippy/installation.html

## License

By contributing to archival, you agree that your contributions will be licensed under its [Unlicense](https://github.com/jesseditson/archival/blob/main/LICENSE.md).
