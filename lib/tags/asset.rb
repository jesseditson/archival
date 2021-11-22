# frozen_string_literal: true

require 'liquid'

class Asset < Liquid::Tag
  # Adds an `asset` tag to liquid. Usage:
  #
  # {% asset "path/to/asset.png" %}
  #
  # This will replace the tag with a relative path to the asset from the
  # current template. Using normal tags will work from the root, but when
  # building dynamic pages or reusing layouts, asset paths are dynamic and
  # will need to be rewritten.

  prepend Liquid::Tag::Disableable

  SYNTAX = /(#{Liquid::QuotedFragment}+)/o.freeze

  def initialize(tag_name, markup, tokens)
    super
    raise AssetError, 'Invalid layout syntax' unless markup =~ SYNTAX

    @path = parse_expression(Regexp.last_match(1))
    @root_dir = Dir.pwd
  end

  def render_to_output_buffer(context, output)
    unless context.key? 'template_path'
      raise AssetError,
            'template_path must be provided to parse when using assets'
    end

    template_path = File.dirname(context['template_path'])
    abs_asset_path = Pathname.new(File.join(@root_dir, @path))
    output << abs_asset_path.relative_path_from(template_path).to_s
    output
  end
end

class AssetError < Liquid::Error
end
