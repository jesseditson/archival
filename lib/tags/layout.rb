# frozen_string_literal: true

require 'liquid'

class Layout < Liquid::Tag
  # Adds a `layout` tag to liquid. Usage:
  #
  # {% layout [layout-name], "key" => "value" %}
  #
  # This will wrap the entire current template in the specified layout, with
  # the arguments provided as local variables.

  prepend Liquid::Tag::Disableable
  @@layout_cache = {}

  SYNTAX = /(#{Liquid::QuotedFragment}+)/o.freeze

  attr_reader :layout_name_expr, :attributes

  def self.reset_cache
    @@layout_cache = {}
  end

  def initialize(tag_name, markup, tokens)
    super

    @page_content = []
    raise LayoutError, 'Invalid layout syntax' unless markup =~ SYNTAX

    layout_name = Regexp.last_match(1)
    @layout_name_expr = parse_expression(layout_name)
    @attributes = {}

    markup.scan(Liquid::TagAttributes) do |key, value|
      @attributes[key] = parse_expression(value)
    end
  end

  def layout_path
    base_path = Dir.pwd
    layout_dir = 'layout'
    layout_path = File.join(base_path, layout_dir)
    unless File.exist? layout_path
      raise LayoutError, "Layout dir #{layout_path} not found"
    end

    layout_path
  end

  def load_layout(layout_name)
    return @@layout_cache[layout_name] if @@layout_cache[layout_name]

    found_layout = nil
    Dir.entries(layout_path).each do |f|
      next unless File.file? File.join(
        layout_path, f
      )

      next unless File.basename(f, '.*') == layout_name
      if found_layout
        raise LayoutError, "More than one layout named #{layout_name} found."
      end

      found_layout = File.join(layout_path, f)
    end
    if found_layout.nil?
      raise LayoutError, "No layouts named #{layout_name} found."
    end

    layout = File.read(found_layout)
    @@layout_cache[layout_name] =
      Liquid::Template.parse(layout)
    @@layout_cache[layout_name]
  end

  def parse(tokens)
    t = tokens.shift
    while t
      @page_content.push t
      t = tokens.shift
    end
  end

  def render_to_output_buffer(context, output)
    layout_name = context.evaluate(@layout_name_expr)
    raise LayoutError, 'Bad layout name argument' unless layout_name

    layout = load_layout(layout_name)

    old_template_name = context.template_name
    old_partial       = context.partial
    begin
      context.template_name = layout_name
      context.partial       = true
      context.stack do
        @attributes.each do |key, value|
          context[key] = context.evaluate(value)
        end
        rendered_page = Liquid::Template.parse(@page_content.join)
                                        .render(context)
        context['page_content'] = rendered_page
        layout.render_to_output_buffer(context,
                                       output)
      end
    ensure
      context.template_name = old_template_name
      context.partial       = old_partial
    end

    output
  end
end

class LayoutError < Liquid::Error
end
