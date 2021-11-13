# frozen_string_literal: true

module Archival
  class TemplateArray < Array
    alias subscript_access []
    alias subscript_write []=

    def initialize(*args)
      super(*args)
      @data = {}
    end

    def [](*args)
      key = args[0]
      return @data[key] if key.is_a? String
      return @data[key] if key.is_a? Symbol

      subscript_access(*args)
    end

    def []=(*args)
      key = args[0]
      if key.is_a?(String) || key.is_a?(Symbol)
        @data[key] = args[1]
        return
      end
      subscript_write(*args)
    end

    def key?(key)
      @data.key?(key)
    end
  end
end
