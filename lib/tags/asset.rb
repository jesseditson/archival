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

  SYNTAX = /(#{Liquid::QuotedFragment}+|\w+)/o.freeze

  def initialize(tag_name, markup, tokens)
    super
    raise AssetError, 'Invalid layout syntax' unless markup =~ SYNTAX

    @path = parse_expression(Regexp.last_match(1))
    # This is defaulted to the pages dir, because it represents the structure
    # of our website. Asset directories are copied as siblings at runtime.
    @@root_dir ||= File.join(Dir.pwd, 'pages')

    @attributes = {}

    markup.scan(Liquid::TagAttributes) do |key, value|
      @attributes[key] = parse_expression(value)
    end
  end

  def self.root_dir=(dir)
    @@root_dir = dir
  end

  def self.helper_port=(port)
    @@helper_port = port
  end

  def render_to_output_buffer(context, output)
    path = @path
    path = path.evaluate(context) if path.is_a? Liquid::VariableLookup
    unless @@root_dir
      raise AssetError,
            'root_dir must be set on Archival::Asset'
    end

    unless context.key? 'template_path'
      raise AssetError,
            'template_path must be provided to parse when using assets'
    end
    template_path = File.dirname(context['template_path'])
    abs_asset_path = Pathname.new(File.join(@@root_dir, path))
    asset_path = abs_asset_path.relative_path_from(template_path).cleanpath.to_s
    output << if @attributes['serve'] == true
                "http://localhost:#{@@helper_port}/#{asset_path}"
              else
                asset_path
              end
    output
  end
end

class AssetError < Liquid::Error
end
