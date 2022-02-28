# frozen_string_literal: true

require 'redcarpet'

module Archival
  class MarkdownRenderer < Redcarpet::Render::HTML
    def autolink(link, _opts)
      # TODO: handle link_type?
      "<a href=\"#{rewrite_link(link)}\">#{rewrite_link(link)}</a>"
    end

    def link(link, title, content)
      "<a href=\"#{rewrite_link(link)}\" title=\"#{title}\">#{content}</a>"
    end

    def image(link, title, alt_text)
      "<img src=\"#{rewrite_link(link)}\" \
title=\"#{title}\" alt=\"#{alt_text}\"/>"
    end

    private

    def rewrite_link(link)
      if link.start_with?('http') || link.start_with?('mailto') ||
         link.start_with?('www') || link.start_with?('/')
        link
      end
      template_dir = File.dirname(@options[:template_file])
      resolved_link = Pathname.new(File.join(@options[:pages_root], link))
      resolved_link.relative_path_from(template_dir)
    end
  end
end
