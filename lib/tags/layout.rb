require 'liquid'


class Layout < Liquid::Tag
  prepend Liquid::Tag::Disableable
  @@layout_cache = {}

  SYNTAX = /(#{Liquid::QuotedFragment}+)/o

  attr_reader :layout_name_expr, :attributes

  def initialize(tag_name, markup, tokens)
      super

      @page_content = []

      if markup =~ SYNTAX

        layout_name = Regexp.last_match(1)
        @layout_name_expr = parse_expression(layout_name)
        @attributes = {}

        markup.scan(Liquid::TagAttributes) do |key, value|
          @attributes[key] = parse_expression(value)
        end

      else
        raise "Invalid layout syntax"
      end
  end

  def load_layout(context)
    layout_name = context.evaluate(@layout_name_expr)
    raise "Bad layout name argument" unless layout_name
    if @@layout_cache[layout_name]
      return @@layout_cache[layout_name]
    end
    base_path = Dir.pwd
    layout_dir = "layout"
    layout_path = File.join(base_path, layout_dir)
    if !File.exist? layout_path then raise "Layout dir #{layout_path} not found" end
    found_layout = nil
    for f in Dir.entries(layout_path)
      if !File.file? File.join(layout_path, f)
        next
      end
      if File.basename(f, ".*") == layout_name
        if found_layout
          raise "More than one layout named #{layout_name} found."
        end
        found_layout = File.join(layout_path, f)
      end
    end
    if found_layout == nil then
      raise "No layouts named #{layout_name} found."
    end
    layout = File.read(found_layout)
    @@layout_cache[layout_name] = Liquid::Template.parse(layout)
    return @@layout_cache[layout_name]
  end

  def parse(tokens)
    while t = tokens.shift() do
      @page_content.push t
    end
  end

  def render_to_output_buffer(context, output)
    layout_name = context.evaluate(@layout_name_expr)
    layout = load_layout(context)

    old_template_name = context.template_name
    old_partial       = context.partial
    begin
      context.template_name = layout_name
      context.partial       = true
      context.stack do
        @attributes.each do |key, value|
          context[key] = context.evaluate(value)
        end
        context['page_content'] = @page_content.map { |content|
          Liquid::Template.parse(content).render(context)
        }
        layout.render_to_output_buffer(context, output)
      end
    ensure
      context.template_name = old_template_name
      context.partial       = old_partial
    end

    output
  end
end