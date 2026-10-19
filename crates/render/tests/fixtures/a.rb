class Batch
  def done?(rows) = rows.empty? && @count.zero?
end
