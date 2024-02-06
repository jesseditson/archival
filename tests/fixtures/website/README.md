# archival-website

A template repo for making archival websites

## What do you need to install on your computer to use this repo?

- Git: https://git-scm.com

This is the version control system we'll use to edit the website. You'll need it locally - you can see if you already have it by opening a terminal and running `git --version`

- rust: https://rustup.rs/

Archival is a cargo crate, so the easiest way to use it is to install via cargo.

- archival

`cargo install archival`

## How to build a website with this repo

1. Click the "Use this template" button at the top of this repo in github:

![Use this template button](https://archival-website-assets.s3.us-west-2.amazonaws.com/use-this-template.png)

2. Clone the resulting repo using `git clone <your-repo-url>`

![Clone button](https://archival-website-assets.s3.us-west-2.amazonaws.com/clone-url.png)

5. Open `dist/index.html` in a browser window

6. Edit the files in this repo, and see your website update in real time

## How to deploy a website from a copy of this repo using netlify

1. Set up a new site in netlify

2. Connect your site to your copy of this repository

3. Add `bin/archival build` to the build steps for the repo

4. Configure netlify's DNS to use your domain name

# More questions?

Head to https://archival.dev for more documentation and help.
