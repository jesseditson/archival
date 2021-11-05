# Archival

https://archival.dev

[![Rspec tests](https://github.com/jesseditson/archival/actions/workflows/rspec.yml/badge.svg)](https://github.com/jesseditson/archival/actions/workflows/rspec.yml)

## Why?

Websites have become a bit homogenous, and basic web skills have become fairly accessible. This project aims to make it very easy to build ultra-durable websites that will last a very long time with little to no maintenance, and that don't rely on trendy build tools.

More musings about how this came about on my blog:

https://jesseditson.com/the-simplest-cms-part-1

## Installation

Add this line to your application's Gemfile:

```ruby
gem 'archival'
```

And then execute:

    $ bundle install

Or install it yourself as:

    $ gem install archival

## Usage

First, install archival by running `gem install archival`.

Then, create a new website:

`archival create my-website`

This sets up some basics. To see your local website, run `archival run`.

When you push an archival website to github, it will automatically build itself to the `dist` folder. To see this locally, run `archival build`.

For more information and documentation, check out the docs at https://archival.dev

Archival uses as few domain specific languages as possible, but doesn't reinvent the wheel. Outside of web standards, here are the technologies used in Archival:

- https://shopify.github.io/liquid/basics/introduction/
- https://github.com/toml-lang/toml/wiki

## Development

After checking out the repo, run `bin/setup` to install dependencies. Then, run `rake spec` to run the tests.

To install this gem onto your local machine, run `bundle exec rake install`. To release a new version, update the version number in `version.rb`, and then run `bundle exec rake release`, which will create a git tag for the version, push git commits and the created tag, and push the `.gem` file to [rubygems.org](https://rubygems.org).

## Contributing

Bug reports and pull requests are welcome on GitHub at https://github.com/jesseditson/archival.
