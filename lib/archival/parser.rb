# frozen_string_literal: true

require 'liquid'
require 'redcarpet'

module Archival
  class Parser
    def initialize(pages_root)
      @pages_root = pages_root
    end

    def parse_object(object, definition, template_file)
      markdown = Redcarpet::Markdown.new(
        Archival::MarkdownRenderer.new(prettify: true,
                                       hard_wrap: true),
        no_intra_emphasis: true,
        fenced_code_blocks: true,
        autolink: true,
        strikethrough: true,
        underline: true,
        template_file: template_file,
        pages_root: @pages_root
      )
      definition.each do |name, type|
        case type
        when 'markdown'
          object[name] = markdown.render(object[name]) if object.key? name
        end
      end
      object
    end
  end
end
